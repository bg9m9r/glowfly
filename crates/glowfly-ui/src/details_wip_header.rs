//! The Unstaged and Staged Files lists' headers (and the plain header row every list of the WIP
//! panel shares, `frame`): the title and the list's quiet Small buttons on the right. Unstaged
//! Files shows Stage all, with Discard all and New file… in its more-actions menu (⋯). As the
//! room shrinks (`fit`), the buttons first drop their labels for Small icon-only squares, then
//! the title is cut with "…" (whole on hover) before its count, which is never cut, and last a
//! button that doesn't fit beside the shortest title moves into the menu, so none is ever cut.

use super::{RowContext, discard_all_text};
use crate::access;
use crate::common::{menu_danger, menu_item};
use crate::theme::color;
use crate::theme::sizes::ControlSize;
use crate::theme::type_scale;
use crate::widgets::buttons::{self, StyledButton};
use crate::widgets::menu::{self, separator};
use crate::widgets::{Icon, truncate};
use eframe::egui::{self, Align, Color32, Layout, Response, Sense, Ui, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::FileListMode;
use glowfly_core::undo::DISCARD_STASH_MESSAGE;
use glowfly_model::keymap::{BindingId, Keymap};
use glowfly_model::msg::{Msg, StageMsg, WorkFileMsg};

/// The size of the header's buttons, the more-actions button and the icon-only squares.
const SIZE: ControlSize = ControlSize::Small;
/// The most of the title's width the buttons keep free before moving into the menu (more when
/// its count needs it).
const TITLE_MIN: f32 = 72.0;
/// A list header's height.
pub(super) const HEADER_H: f32 = 32.0;

/// One of the header's buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
enum HeaderAction {
    StageAll,
    DiscardAll,
    NewFile,
    UnstageAll,
}

impl HeaderAction {
    fn label(self, t: Lang) -> &'static str {
        match self {
            Self::StageAll => t.tr("details.wip.stage_all.button"),
            Self::DiscardAll => t.tr("details.wip.discard_all.button"),
            Self::NewFile => t.tr("details.wip.new_file.button"),
            Self::UnstageAll => t.tr("details.wip.unstage_all.button"),
        }
    }

    /// The button's hover text, with its key.
    fn tip(self, keys: &Keymap, t: Lang) -> Option<String> {
        match self {
            Self::StageAll => Some(keys.tip(t.tr("details.wip.stage_all.tip"), BindingId::StageAll)),
            Self::UnstageAll => Some(keys.tip(t.tr("details.wip.unstage_all.tip"), BindingId::UnstageAll)),
            Self::DiscardAll | Self::NewFile => None,
        }
    }

    /// The button: quiet and small, as every action in a list header.
    fn button<'a>(self, t: Lang, tip: Option<&'a str>) -> StyledButton<'a> {
        let b = buttons::quiet(self.label(t)).size(ControlSize::Small);
        match tip {
            Some(tip) => b.tip(tip),
            None => b,
        }
    }

    /// The icon its icon-only square shows when the title has no room for its label.
    fn icon(self) -> Icon {
        match self {
            Self::StageAll => Icon::Plus,
            Self::UnstageAll => Icon::Minus,
            Self::NewFile => Icon::FileAdd,
            Self::DiscardAll => Icon::Close,
        }
    }

    /// The button with its label, or as an icon-only square named by it.
    fn shown<'a>(self, t: Lang, tip: Option<&'a str>, icon: bool) -> StyledButton<'a> {
        if !icon {
            return self.button(t, tip);
        }
        let b = square(self.icon(), self.label(t));
        match tip {
            Some(tip) => b.tip(tip),
            None => b,
        }
    }

    fn msg(self, t: Lang, keys: &Keymap) -> Msg {
        match self {
            Self::StageAll => Msg::Stage(StageMsg::StageAll),
            Self::UnstageAll => Msg::Stage(StageMsg::UnstageAll),
            Self::NewFile => Msg::WorkFile(WorkFileMsg::NewFileDialog(String::new())),
            // Discarding goes through a stash so it can be recovered.
            Self::DiscardAll => Msg::confirm_danger(
                t.tr("details.wip.discard_all.title"),
                discard_all_text(t, keys),
                t.tr("details.wip.discard_all.button"),
                Msg::git("Discard all changes", &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]),
            ),
        }
    }
}

/// The header's buttons, rightmost first: Unstage all on Staged Files; Stage all and Discard all
/// (when there are files) and New file… (always) on Unstaged Files.
fn actions(staged: bool, count: usize) -> Vec<HeaderAction> {
    use HeaderAction::*;
    match (staged, count > 0) {
        (true, true) => vec![UnstageAll],
        (true, false) => Vec::new(),
        (false, true) => vec![StageAll, DiscardAll, NewFile],
        (false, false) => vec![NewFile],
    }
}

/// A header row's room: `avail` px for a title `title_w` wide (`title_min` when cut) and its
/// buttons, `gap` apart; the more-actions button takes `more_w` (its gap included) and an
/// icon-only button `icon_w`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Room {
    pub avail: f32,
    pub title_w: f32,
    pub title_min: f32,
    pub gap: f32,
    pub more_w: f32,
    pub icon_w: f32,
}

/// What a header shows: its first `shown` buttons, as icon-only squares when `icons`; the rest
/// are in its more-actions menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fit {
    pub shown: usize,
    pub icons: bool,
}

/// Fits the first `most` of the buttons (labelled `labels` wide, rightmost first) beside the
/// title in `r`, giving way in this order: the labels go (icon-only squares), then the title is
/// cut down to `title_min`, and only then do buttons move into the menu, the leftmost first.
pub(crate) fn fit(r: Room, labels: &[f32], most: usize) -> Fit {
    let want = most.min(labels.len());
    let more = |k: usize| if k < labels.len() { r.more_w } else { 0.0 };
    let labelled = labels[..want].iter().sum::<f32>() + r.gap * want as f32;
    if labelled + more(want) + r.title_w <= r.avail {
        return Fit { shown: want, icons: false };
    }
    let icons = |k: usize| (r.icon_w + r.gap) * k as f32 + more(k);
    let title = r.title_w.min(r.title_min);
    let shown = (0..=want).rev().find(|&k| icons(k) + title <= r.avail).unwrap_or(0);
    Fit { shown, icons: true }
}

/// A header action's Small icon-only square named `name`. Its glyph spans about 12 of the
/// square's 14 px icon box, like the check and ⋯ beside it: the plus and minus are drawn with
/// 6 px arms, and the undo arrow smaller, as it fills more of its grid.
pub(crate) fn square<'a>(icon: Icon, name: &'a str) -> StyledButton<'a> {
    let b = buttons::icon_only(icon, name).size(SIZE);
    match icon {
        Icon::Plus | Icon::Minus => b.icon_px(GLYPH_SPAN * 2.0),
        Icon::Undo => b.icon_px(GLYPH_SPAN * 0.875),
        _ => b,
    }
}

/// The extent a header square's glyph spans, inside its size's 14 px icon box.
const GLYPH_SPAN: f32 = 12.0;

/// The least a header's title is cut to: `TITLE_MIN`, or its count and a few characters more.
fn title_min(ui: &Ui, title: &str) -> f32 {
    let (_, count) = truncate::split_count(title);
    let font = type_scale::section(ui);
    let count_w = ui.fonts(|f| f.layout_no_wrap(count.to_owned(), font, Color32::PLACEHOLDER).size().x);
    TITLE_MIN.max(count_w + TITLE_MIN / 2.0)
}

/// A header's `title`, cut before its count with "…" (whole on hover) to the room left. Not
/// selectable, so a right-click on it reaches the header's menu.
pub(crate) fn title(ui: &mut Ui, title: &str) {
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        truncate::cut_counted_label(ui, title, type_scale::section(ui), color::text(), ui.available_width());
    });
}

/// The room of a header row in `ui` for `title`, with its buttons Small.
pub(crate) fn room(ui: &Ui, title: &str) -> Room {
    let gap = ui.spacing().item_spacing.x;
    let section = type_scale::section(ui);
    let title_w = ui.fonts(|f| f.layout_no_wrap(title.to_owned(), section, Color32::PLACEHOLDER).size().x);
    let icon_w = SIZE.metrics().h;
    Room { avail: ui.available_width(), title_w, title_min: title_min(ui, title), gap, more_w: icon_w + gap, icon_w }
}

/// The header of the Unstaged (or `staged`) Files list showing `title`, with `count` files.
/// Returns the header's response, for the Staged header's menu.
pub(super) fn header(ui: &mut Ui, title: String, staged: bool, count: usize, ctx: RowContext, msgs: &mut Vec<Msg>) -> Response {
    // The Staged header's background opens its menu; registered before the header's buttons, so
    // they keep their own clicks.
    let sense = if staged { Sense::click() } else { Sense::hover() };
    let scope = ui.scope_builder(egui::UiBuilder::new().sense(sense), |ui| {
        frame(ui, |ui| {
            // Rightmost; the buttons measure the room left after it.
            let toggle = crate::list_row::file_tree::mode_toggle(ui, ctx.lang, ctx.mode, msgs);
            // Two lists show the toggle: the name says which one it changes.
            let name = match (staged, ctx.mode) {
                (false, FileListMode::List) => ctx.lang.tr("files.tree.unstaged.show_tree.name"),
                (false, FileListMode::Tree) => ctx.lang.tr("files.tree.unstaged.show_list.name"),
                (true, FileListMode::List) => ctx.lang.tr("files.tree.staged.show_tree.name"),
                (true, FileListMode::Tree) => ctx.lang.tr("files.tree.staged.show_list.name"),
            };
            access::name_button(&toggle, true, name);
            buttons(ui, &title, staged, count, ctx, msgs);
            // Drawn after the buttons, so a long title is cut short instead of running under them.
            self::title(ui, &title);
        });
    });
    scope.response
}

/// A list header's row: plain on the panel (no bar), `HEADER_H` tall, laid out right to left so
/// `add` places the buttons first and the title in the room left.
pub(crate) fn frame(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let size = vec2(ui.available_width(), HEADER_H);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
        ui.set_min_size(size);
        add(ui);
    });
}

/// The header's buttons, right to left, with the more-actions button rightmost when some don't
/// fit beside `title`.
fn buttons(ui: &mut Ui, title: &str, staged: bool, count: usize, ctx: RowContext, msgs: &mut Vec<Msg>) {
    let (keys, t) = (ctx.keys, ctx.lang);
    let actions = actions(staged, count);
    let tips: Vec<Option<String>> = actions.iter().map(|a| a.tip(keys, t)).collect();
    // Unstaged Files keeps only Stage all out: Discard all and New file… wait in the menu.
    let most = if staged { actions.len() } else { 1 };
    let Fit { shown, icons } = fit(room(ui, title), &widths(ui, &actions, t), most);
    if shown < actions.len() {
        more_button(ui, &actions[shown..], ctx, msgs);
    }
    for (a, tip) in actions[..shown].iter().zip(&tips) {
        if ui.add(a.shown(t, tip.as_deref(), icons)).clicked() {
            msgs.push(a.msg(t, keys));
        }
    }
}

/// The widths of `actions`' buttons in `ui`, as `StyledButton` lays them out.
fn widths(ui: &Ui, actions: &[HeaderAction], t: Lang) -> Vec<f32> {
    actions.iter().map(|a| a.button(t, None).desired_width(ui)).collect()
}

/// The more-actions button and its menu, listing `moved` in reading order,
/// with Discard all… last, after a separator, in the danger style.
fn more_button(ui: &mut Ui, moved: &[HeaderAction], ctx: RowContext, msgs: &mut Vec<Msg>) {
    let (keys, t) = (ctx.keys, ctx.lang);
    more_menu(ui, t, |ui| {
        for a in moved.iter().rev().filter(|a| **a != HeaderAction::DiscardAll) {
            menu_item(msgs, ui, a.label(t), a.msg(t, keys));
        }
        if moved.contains(&HeaderAction::DiscardAll) {
            separator(ui);
            let a = HeaderAction::DiscardAll;
            menu_danger(msgs, ui, t.tr("details.wip.discard_all.item"), None, a.msg(t, keys));
        }
    });
}

/// A header's more-actions button (a Small icon-only ⋯) opening a menu of `items` below it.
pub(crate) fn more_menu(ui: &mut Ui, t: Lang, items: impl FnOnce(&mut Ui)) {
    let r = ui.add(buttons::icon_only(Icon::More, t.tr("details.wip.more.tip")).size(SIZE));
    menu::below(ui, &r, items);
}

#[cfg(test)]
#[path = "details_wip_header_tests.rs"]
mod tests;
