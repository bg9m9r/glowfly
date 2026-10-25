//! The Unstaged and Staged Files lists' headers (and the plain header row every list of the WIP
//! panel shares, `frame`): the title, cut with "…" (whole on hover), and the list's quiet
//! buttons on the right. Unstaged Files shows Stage all, with Discard all and New file… in its
//! more-actions menu; and a button that doesn't fit beside the title moves into that menu too
//! (Stage all last), so none is ever cut.

use super::{RowContext, discard_all_text};
use crate::keymap::{BindingId, Keymap};
use crate::msg::{Msg, StageMsg, WorkFileMsg};
use crate::ui::access;
use crate::ui::common::menu_item;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons::{self, StyledButton};
use crate::ui::widgets::{self, Icon, truncate};
use eframe::egui::{self, Align, Color32, Layout, Response, Sense, Ui, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::undo::DISCARD_STASH_MESSAGE;

/// The width of the more-actions button.
const MORE_W: f32 = 24.0;
/// The most of the title's width the buttons keep free before moving into the menu.
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
        let b = buttons::quiet(self.label(t)).small();
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

/// How many of the buttons (`widths`, rightmost first) are shown in `avail` px beside at least
/// `title_min` px of title, `gap` px apart. All when they fit; otherwise as many as fit beside
/// the more-actions button (`more_w`, its gap included), the rest moving into its menu.
pub(super) fn shown_count(avail: f32, title_min: f32, widths: &[f32], more_w: f32, gap: f32) -> usize {
    let row = |k: usize| widths[..k].iter().sum::<f32>() + gap * k as f32;
    if row(widths.len()) + title_min <= avail {
        return widths.len();
    }
    (0..widths.len()).rev().find(|&k| row(k) + more_w + title_min <= avail).unwrap_or(0)
}

/// How many buttons to show, given `cut` (beside a title cut to `TITLE_MIN`) and `whole` (beside
/// the whole title) of `len`: when every button fits only by cutting the title, the last one
/// (New file…) moves into the menu if that shows the title whole.
fn whole_title_count(cut: usize, whole: usize, len: usize) -> usize {
    if cut == len && whole + 1 == len && whole > 0 { whole } else { cut }
}

/// The header of the Unstaged (or `staged`) Files list showing `title`, with `count` files.
/// Returns the header's response, for the Staged header's menu.
pub(super) fn header(ui: &mut Ui, title: String, staged: bool, count: usize, ctx: RowContext, msgs: &mut Vec<Msg>) -> Response {
    // The Staged header's background opens its menu; registered before the header's buttons, so
    // they keep their own clicks.
    let sense = if staged { Sense::click() } else { Sense::hover() };
    let scope = ui.scope_builder(egui::UiBuilder::new().sense(sense), |ui| {
        frame(ui, |ui| {
            buttons(ui, &title, staged, count, ctx, msgs);
            // Drawn after the buttons, so a long title is cut short (whole on hover) instead of
            // running under them. Not selectable, so a right-click on the title reaches the
            // header's menu.
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let text = type_scale::section_text(ui, title);
                truncate::truncate_label(ui, text, ui.available_width());
            });
        });
    });
    scope.response
}

/// A list header's row: plain on the panel (no bar), `HEADER_H` tall, laid out right to left so
/// `add` places the buttons first and the title in the room left.
pub(in crate::ui) fn frame(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
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
    let gap = ui.spacing().item_spacing.x;
    let widths = widths(ui, &actions, t);
    let section = type_scale::section(ui);
    let title_w = ui.fonts(|f| f.layout_no_wrap(title.to_owned(), section, Color32::PLACEHOLDER).size().x);
    let avail = ui.available_width();
    let cut = shown_count(avail, title_w.min(TITLE_MIN), &widths, MORE_W + gap, gap);
    let shown = whole_title_count(cut, shown_count(avail, title_w, &widths, MORE_W + gap, gap), widths.len());
    // Unstaged Files keeps only Stage all out: Discard all and New file… wait in the menu.
    let shown = if staged || actions.len() == 1 { shown } else { shown.min(1) };
    if shown < actions.len() {
        more_button(ui, &actions[shown..], ctx, msgs);
    }
    for (a, tip) in actions[..shown].iter().zip(&tips) {
        if ui.add(a.button(t, tip.as_deref())).clicked() {
            msgs.push(a.msg(t, keys));
        }
    }
}

/// The widths of `actions`' buttons in `ui`, as `StyledButton` lays them out.
fn widths(ui: &Ui, actions: &[HeaderAction], t: Lang) -> Vec<f32> {
    actions.iter().map(|a| a.button(t, None).desired_width(ui)).collect()
}

/// The more-actions button (a painted icon) and its menu, listing `moved` in reading order.
fn more_button(ui: &mut Ui, moved: &[HeaderAction], ctx: RowContext, msgs: &mut Vec<Msg>) {
    let (keys, t) = (ctx.keys, ctx.lang);
    more_menu(ui, t, |ui| {
        for a in moved.iter().rev() {
            menu_item(msgs, ui, a.label(t), a.msg(t, keys));
        }
    });
}

/// A header's more-actions button (a painted icon) opening a menu of `items`.
pub(in crate::ui) fn more_menu(ui: &mut Ui, t: Lang, items: impl FnOnce(&mut Ui)) {
    let button = egui::Button::new("").small().frame(false).min_size(vec2(MORE_W, 0.0));
    let menu = egui::menu::menu_custom_button(ui, button, items);
    let tip = t.tr("details.wip.more.tip");
    let r = menu.response.on_hover_text(tip);
    widgets::paint_icon(ui.painter(), r.rect.center(), Icon::More, color::text());
    access::name_button(&r, true, tip);
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: f32 = 8.0;
    const MORE: f32 = 32.0;
    /// Stage all, Discard all, New file….
    const THREE: [f32; 3] = [70.0, 80.0, 60.0];

    fn all_w(widths: &[f32], title: f32) -> f32 {
        widths.iter().sum::<f32>() + GAP * widths.len() as f32 + title
    }

    #[test]
    fn every_button_shows_when_they_fit() {
        assert_eq!(shown_count(all_w(&THREE, 72.0), 72.0, &THREE, MORE, GAP), 3);
        assert_eq!(shown_count(1000.0, 72.0, &THREE, MORE, GAP), 3);
    }

    #[test]
    fn one_px_short_moves_new_file_and_leaves_room_for_the_menu() {
        let avail = all_w(&THREE, 72.0) - 1.0;
        let k = shown_count(avail, 72.0, &THREE, MORE, GAP);
        assert_eq!(k, 2, "New file… moves first");
        assert!(all_w(&THREE[..k], 72.0) + MORE <= avail, "the menu button fits");
        // Narrower still, Discard all and then Stage all follow.
        assert_eq!(shown_count(all_w(&THREE[..1], 72.0) + MORE, 72.0, &THREE, MORE, GAP), 1);
    }

    #[test]
    fn new_file_moves_to_the_menu_to_show_the_whole_title() {
        assert_eq!(whole_title_count(3, 2, 3), 2, "one button fewer shows the title whole");
        assert_eq!(whole_title_count(3, 3, 3), 3, "all fit beside the whole title");
        assert_eq!(whole_title_count(3, 1, 3), 3, "a long title is cut rather than lose two buttons");
        assert_eq!(whole_title_count(2, 1, 3), 2, "already short of room: the usual rule");
        assert_eq!(whole_title_count(1, 0, 1), 1, "a lone button stays");
    }

    #[test]
    fn very_narrow_shows_only_the_menu() {
        assert_eq!(shown_count(60.0, 72.0, &THREE, MORE, GAP), 0);
        assert_eq!(shown_count(0.0, 0.0, &THREE, MORE, GAP), 0);
    }

    #[test]
    fn the_title_keeps_its_least_width() {
        let avail = all_w(&THREE, 0.0) + 40.0;
        assert_eq!(shown_count(avail, 40.0, &THREE, MORE, GAP), 3, "a short title leaves room for all");
        assert!(shown_count(avail, 72.0, &THREE, MORE, GAP) < 3, "a longer one moves some into the menu");
    }

    #[test]
    fn a_single_button_moves_only_when_it_alone_does_not_fit() {
        let one = [90.0];
        assert_eq!(shown_count(90.0 + GAP + 72.0, 72.0, &one, MORE, GAP), 1);
        assert_eq!(shown_count(90.0 + GAP + 71.0, 72.0, &one, MORE, GAP), 0);
        assert_eq!(shown_count(1000.0, 72.0, &[], MORE, GAP), 0, "no buttons, no menu");
    }

    #[test]
    fn widths_are_the_styled_buttons_own() {
        let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
            crate::ui::theme::apply_theme(ui.ctx());
            let t = Lang::En;
            let all = actions(false, 2);
            let w = widths(ui, &all, t);
            for (a, w) in all.iter().zip(&w) {
                assert_eq!(ui.add(a.button(t, None)).rect.width(), *w, "{a:?} is as wide as it said");
                assert_eq!(ui.add(a.button(t, None)).rect.height(), 24.0, "{a:?} is small");
            }
            // One px short of all three beside the title: New file… moves into the menu first.
            let gap = ui.spacing().item_spacing.x;
            let row = w.iter().sum::<f32>() + gap * 3.0 + TITLE_MIN;
            assert_eq!(shown_count(row, TITLE_MIN, &w, MORE_W + gap, gap), 3);
            assert_eq!(shown_count(row - 1.0, TITLE_MIN, &w, MORE_W + gap, gap), 2);
        });
        h.run();
    }

    #[test]
    fn actions_follow_the_list_and_its_files() {
        use HeaderAction::*;
        assert_eq!(actions(true, 2), [UnstageAll]);
        assert!(actions(true, 0).is_empty());
        assert_eq!(actions(false, 2), [StageAll, DiscardAll, NewFile]);
        assert_eq!(actions(false, 0), [NewFile]);
    }
}
