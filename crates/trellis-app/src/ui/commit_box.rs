//! The commit box under the WIP file lists: summary and description, the Amend, "Push after
//! committing" and "Skip hooks" options, the commit button, and the Ctrl+Enter shortcuts. The
//! summary takes the keyboard when `view.focus_message` is set (Ctrl+Shift+M).

use super::UiCache;
use super::common::issue_chip;
use super::spell_edit::spell_text_edit;
use super::theme::color;
use crate::keymap::{BindingId, Keymap};
use crate::msg::CommitMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::commit::{commit_blocked, pushes_after};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Key, Layout, Modifiers, RichText};
use trellis_core::i18n::Lang;

/// Height the commit box needs below the file lists (the lists share what is left).
pub(super) const RESERVED_HEIGHT: f32 = 340.0;

/// The commit button's tooltip: the keys that commit from the message fields.
pub(super) fn shortcuts_tip(t: Lang, keys: &Keymap) -> String {
    match (keys.hint(BindingId::Commit), keys.hint(BindingId::StageAllAndCommit)) {
        (Some(c), Some(a)) => t.trf("commit.shortcuts.both", &[("commit", &c), ("all", &a)]),
        (Some(c), None) => t.trf("commit.shortcuts.commit_only", &[("commit", &c)]),
        (None, Some(a)) => t.trf("commit.shortcuts.all_only", &[("all", &a)]),
        (None, None) => t.tr("commit.shortcuts.none").to_owned(),
    }
}

pub(super) fn show(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    ui.add_space(6.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(type_scale::section_text(ui, t.tr("commit.title")));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if controls::checkbox(ui, &mut st.draft.amend, t.tr("commit.amend")).changed() && st.draft.amend {
                msgs.push(Msg::Commit(CommitMsg::PrefillAmend));
            }
        });
    });

    // Link the issue encoded in the branch name (e.g. 1234-login or PROJ-12-crash).
    if let Some((fs, key)) = st.forge.as_ref().and_then(|f| Some((f, f.branch_issue(snap.head_branch.as_deref()?)?))) {
        let tag = fs.mention(&key);
        let mentioned = fs.links_issue(&st.draft.summary, &key) || fs.links_issue(&st.draft.description, &key);
        ui.horizontal(|ui| {
            issue_chip(ui, fs, &key, msgs);
            let hover = if fs.message_ref(&key) == tag { t.tr("commit.link_tip.mention") } else { t.tr("commit.link_tip.link") };
            if !mentioned
                && ui.add(buttons::secondary(t.trf("commit.link_button", &[("tag", &tag)])).small()).on_hover_text(hover).clicked()
            {
                msgs.push(Msg::Commit(CommitMsg::AppendToDescription(fs.message_ref(&key))));
            }
        });
    }

    let spell = st.settings.spellcheck;
    let s = spell_text_edit(ui, &mut st.draft.summary, t.tr("commit.summary_hint"), 1, &mut cache.speller, spell);
    super::access::name_field(&s, t.tr("commit.summary_hint"));
    if std::mem::take(&mut st.view.focus_message) {
        s.request_focus();
    }
    let len = st.draft.summary.chars().count();
    let c = if len > 72 { color::red() } else { color::faint() };
    ui.label(RichText::new(format!("{len}/72")).size(type_scale::SMALL).color(c));
    let d = spell_text_edit(ui, &mut st.draft.description, t.tr("commit.description_hint"), 4, &mut cache.speller, spell);
    super::access::name_field(&d, t.tr("commit.description_hint"));
    if let Some(template) = &st.draft.template {
        let note = t.trf("commit.template_note", &[("char", &template.comment_char)]);
        ui.label(RichText::new(note).size(type_scale::SMALL).color(color::faint()));
    }

    ui.horizontal_wrapped(|ui| {
        let mut push = st.settings.push_after_commit;
        let r = ui
            .add_enabled(snap.head_branch.is_some(), controls::Check::new(&mut push, t.tr("commit.push_after.check")))
            .on_hover_text(t.tr("commit.push_after.tip"))
            .on_disabled_hover_text(t.tr("commit.push_after.detached"));
        // The setting changes through update, which also saves it.
        if r.changed() {
            msgs.push(Msg::Commit(CommitMsg::SetPushAfterCommit(push)));
        }
        controls::checkbox(ui, &mut st.draft.skip_hooks, t.tr("commit.skip_hooks.check")).on_hover_text(t.tr("commit.skip_hooks.tip"));
    });

    let label = st.draft.button_label(t, snap.staged.len(), pushes_after(st), snap.state == git2::RepositoryState::Merge);
    let blocked = commit_blocked(st, false);
    let enabled = blocked.is_none();
    let tip = shortcuts_tip(t, &st.keymap);
    let button = buttons::primary(label).enabled(enabled).why(blocked).tip(&tip).min_width(ui.available_width()).min_height(34.0);
    if ui.add(button).clicked() {
        msgs.push(Msg::Commit(CommitMsg::Commit));
    }

    if s.has_focus() || d.has_focus() {
        let (all, one) = ui.input_mut(|i| {
            // Shift first: consume_key ignores an extra Shift, so Ctrl+Enter would match both.
            let all = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter);
            (all, !all && i.consume_key(Modifiers::COMMAND, Key::Enter))
        });
        if all {
            msgs.push(Msg::Commit(CommitMsg::StageAllAndCommit));
        } else if one {
            msgs.push(Msg::Commit(CommitMsg::Commit));
        }
    }
}
