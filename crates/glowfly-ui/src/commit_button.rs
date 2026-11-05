//! The commit box's button: the commit button and its menu caret as one split button. The
//! button always names the action; the caret's menu holds Stage all and commit (with its key),
//! Commit and push (not while "Push after committing" is ticked), Amend last commit (not while
//! Amend is ticked), each greyed with its reason when it can't run, then Skip hooks after a
//! separator. At a rebase stopped to edit the menu holds only Skip hooks. The caret is tinted
//! with the warning tone while hooks are skipped, so the choice stays visible with the menu
//! closed.

use crate::theme::sizes::ControlSize;
use crate::theme::surfaces::Tone;
use crate::widgets::button_split::CARET_W;
use crate::widgets::buttons;
use crate::widgets::menu::{self, popup_above_or_below};
use crate::widgets::menu_row;
use crate::widgets::reason::{ItemState, menu_button};
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{CommitMsg, Msg};
use glowfly_model::state::commit::CommitFields;

/// The least width of the caret's menu.
const MENU_W: f32 = 160.0;

/// One of the menu's actions: its label, its key (empty for none), why it can't run, and what
/// it does.
pub(super) struct MenuAction {
    pub label: String,
    pub key: String,
    pub why: Option<String>,
    pub act: Act,
}

/// What a menu action does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Act {
    StageAllAndCommit,
    CommitAndPush,
    /// Ticks Amend (prefilling a blank draft from the last commit) and focuses the summary.
    Amend,
}

/// The commit button: its label, whether it can be clicked, whether it is the view's primary,
/// the caption (why it can't be clicked) and its tooltip.
pub(super) struct Button<'a> {
    pub label: String,
    pub enabled: bool,
    pub primary: bool,
    pub caption: Option<&'a str>,
    pub tip: &'a str,
}

/// The split button. Returns the commit button's response and the menu action picked, if any.
pub(super) fn show(
    ui: &mut egui::Ui,
    b: Button,
    actions: &[MenuAction],
    draft: &mut CommitFields,
    t: Lang,
    msgs: &mut Vec<Msg>,
) -> (egui::Response, Option<Act>) {
    let popup = ui.make_persistent_id("commit-more");
    let open = ui.memory(|m| m.is_popup_open(popup));
    let skip = draft.skip_hooks;
    let w = ui.available_width() - CARET_W;
    let button = if b.primary { buttons::primary(b.label) } else { buttons::secondary(b.label) };
    let button = button.enabled(b.enabled).why(b.caption).tip(b.tip).min_width(w).max_width(w).size(ControlSize::Large);
    // The caret's name stays the same either way, so it is always found by it.
    let (commit, caret) = buttons::split(button, t.tr("commit.more.tip")).open(open).menu_tone(skip.then_some(Tone::Warning)).show(ui);
    let caret = caret.on_hover_text(if skip { t.tr("commit.more.skipping_tip") } else { t.tr("commit.more.tip") });
    if caret.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    let mut picked = None;
    // Above the caret (the button sits at the bottom of the panel), its right edge on the
    // caret's: the anchor is widened leftwards to the menu's width last frame.
    let width_id = popup.with("width");
    let menu_w = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or(MENU_W);
    let mut anchor = caret.clone();
    anchor.rect.min.x = anchor.rect.max.x - menu_w.max(anchor.rect.width());
    let close = egui::PopupCloseBehavior::CloseOnClickOutside;
    let drawn = popup_above_or_below(ui, popup, &anchor, egui::AboveOrBelow::Above, close, |ui| {
        // Keep the labels on one line, the menu as wide as they need.
        ui.set_min_width(MENU_W);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        for a in actions {
            if menu_button(ui, a.label.as_str(), a.key.as_str(), ItemState::blocked_by(a.why.as_deref())).clicked() {
                picked = Some(a.act);
                ui.memory_mut(|m| m.close_popup());
            }
        }
        menu::separator(ui);
        // A check item, on the items' text column. Closes at once, so the caret's tint shows
        // the choice.
        let check = menu_row::item(t.tr("commit.skip_hooks.check")).check(draft.skip_hooks).show(ui);
        if check.on_hover_text(t.tr("commit.skip_hooks.tip")).clicked() {
            draft.skip_hooks = !draft.skip_hooks;
            ui.memory_mut(|m| m.close_popup());
        }
        ui.min_rect().width()
    });
    // The frame's width: the contents' plus its side margins.
    if let Some(w) = drawn.map(|w| w + 2.0 * ui.style().spacing.menu_margin.leftf()) {
        ui.data_mut(|d| d.insert_temp(width_id, w));
    }
    match picked {
        Some(Act::StageAllAndCommit) => msgs.push(Msg::Commit(CommitMsg::StageAllAndCommit)),
        Some(Act::CommitAndPush) => msgs.push(Msg::Commit(CommitMsg::CommitAndPush)),
        Some(Act::Amend) | None => {}
    }
    (commit, picked)
}
