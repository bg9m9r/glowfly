//! The commit box under the WIP file lists: summary and description, the Amend, "Push after
//! committing" and "Skip hooks" options, the commit button, and the Ctrl+Enter shortcuts. The
//! summary takes the keyboard when `view.focus_message` is set (Ctrl+Shift+M).

use super::UiCache;
use super::common::issue_chip;
use super::spell_edit::spell_text_edit;
use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::commit::{commit_blocked, pushes_after};
use eframe::egui::{self, Align, Color32, Key, Layout, Modifiers, RichText, vec2};

/// Height the commit box needs below the file lists (the lists share what is left).
pub(super) const RESERVED_HEIGHT: f32 = 340.0;

fn shortcuts_tip() -> String {
    use crate::keymap::{BindingId, hint};
    format!("{} commits from the message fields; {} stages all changes first.", hint(BindingId::Commit), hint(BindingId::StageAllAndCommit))
}
const PUSH_HINT: &str =
    "After a successful commit, push the branch. A branch with no upstream is pushed to the default remote and tracks it.";
const NO_PUSH_DETACHED: &str = "Nothing to push from a detached HEAD";
const SKIP_HOOKS_HINT: &str = "Commit with --no-verify: the pre-commit and commit-msg hooks don't run.";

pub(super) fn show(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    ui.add_space(6.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Commit").strong().color(color::text()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.checkbox(&mut st.draft.amend, "Amend").changed() && st.draft.amend {
                msgs.push(Msg::PrefillAmend);
            }
        });
    });

    // Link the issue encoded in the branch name (e.g. 1234-login or PROJ-12-crash).
    if let Some((fs, key)) = st.forge.as_ref().and_then(|f| Some((f, f.branch_issue(snap.head_branch.as_deref()?)?))) {
        let tag = fs.mention(&key);
        let mentioned = st.draft.summary.contains(&tag) || st.draft.description.contains(&tag);
        ui.horizontal(|ui| {
            issue_chip(ui, fs, &key, msgs);
            if !mentioned && ui.small_button(format!("Link {tag}")).on_hover_text("Adds the mention to the description").clicked() {
                msgs.push(Msg::AppendToDescription(tag.clone()));
            }
        });
    }

    let spell = st.settings.spellcheck;
    let s = spell_text_edit(ui, &mut st.draft.summary, "Commit summary", 1, &mut cache.speller, spell);
    if std::mem::take(&mut st.view.focus_message) {
        s.request_focus();
    }
    let len = st.draft.summary.chars().count();
    let c = if len > 72 { color::red() } else { color::faint() };
    ui.label(RichText::new(format!("{len}/72")).small().color(c));
    let d = spell_text_edit(ui, &mut st.draft.description, "Description", 4, &mut cache.speller, spell);
    if let Some(t) = &st.draft.template {
        let note = format!("The template's lines starting with {} are left out of the message", t.comment_char);
        ui.label(RichText::new(note).small().color(color::faint()));
    }

    ui.horizontal_wrapped(|ui| {
        let mut push = st.settings.push_after_commit;
        let r = ui
            .add_enabled(snap.head_branch.is_some(), egui::Checkbox::new(&mut push, "Push after committing"))
            .on_hover_text(PUSH_HINT)
            .on_disabled_hover_text(NO_PUSH_DETACHED);
        // The setting changes through update, which also saves it.
        if r.changed() {
            msgs.push(Msg::SetPushAfterCommit(push));
        }
        ui.checkbox(&mut st.draft.skip_hooks, "Skip hooks").on_hover_text(SKIP_HOOKS_HINT);
    });

    let label = st.draft.button_label(snap.staged.len(), pushes_after(st));
    let blocked = commit_blocked(st, false);
    let enabled = blocked.is_none();
    let button = egui::Button::new(RichText::new(label).strong().color(if enabled { Color32::WHITE } else { color::faint() }))
        .fill(if enabled { color::primary() } else { color::toolbar() })
        .min_size(vec2(ui.available_width(), 34.0));
    let r = ui.add_enabled(enabled, button).on_hover_text(shortcuts_tip()).on_disabled_hover_text(blocked.unwrap_or_default());
    if r.clicked() {
        msgs.push(Msg::Commit);
    }

    if s.has_focus() || d.has_focus() {
        let (all, one) = ui.input_mut(|i| {
            // Shift first: consume_key ignores an extra Shift, so Ctrl+Enter would match both.
            let all = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter);
            (all, !all && i.consume_key(Modifiers::COMMAND, Key::Enter))
        });
        if all {
            msgs.push(Msg::StageAllAndCommit);
        } else if one {
            msgs.push(Msg::Commit);
        }
    }
}
