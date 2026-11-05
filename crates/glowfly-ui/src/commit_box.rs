//! The commit box pinned under the WIP file lists, or inside step 3 of an operation's steps
//! (`BoxStyle`: titled "Commit" in the plain panel; a secondary button beside Continue), top to
//! bottom: the header ("Commit", then the author identity git gives, state/repo_identity.rs, in
//! the warning tone with the variables on hover when `GIT_AUTHOR_*` or `GIT_COMMITTER_*` is set;
//! without the title in step 3), the summary with its length inside its trailing edge from the
//! first character (faint up to 50, weak to 72, amber past it: `state::commit::summary_length`),
//! the description, one row of options (Amend last commit, off with its reason on hover while a
//! merge, cherry-pick or revert is stopped; Push after committing), the commit button
//! (`commit_button.rs`) and its caption: the first missing step, wrapped and never cut (a line's
//! height is always kept, so the box changes height only for a caption that wraps), also the
//! button's description for screen readers. While a rebase is stopped "Push after committing"
//! isn't offered; at a stop to edit the button is Amend and continue (Commit and continue with
//! Amend off), and Amend is off, with its reason, where git wouldn't amend the commit being
//! edited. Ctrl+Enter and Ctrl+Shift+Enter commit from the fields; the summary takes the
//! keyboard when `view.focus_message` is set (Ctrl+Shift+M).

#[path = "commit_button.rs"]
mod button;

use super::UiCache;
use super::common::issue_chip;
use super::spell_edit::{spell_text_edit, spell_text_edit_trailing};
use super::theme::color;
use crate::theme::controls::{self, FIELD_PAD_X};
use crate::theme::sizes::ControlSize;
use crate::theme::type_scale;
use crate::widgets::{buttons, reason, truncate_label};
use button::{Act, MenuAction};
use eframe::egui::{self, Align, Key, Layout, Modifiers, RichText, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_model::keymap::{BindingId, Keymap};
use glowfly_model::msg::CommitMsg;
use glowfly_model::msg::EditMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::commit::stop::{amend_blocked_in, edit_stop, rebase_stopped};
use glowfly_model::state::commit::{CommitFields, SUMMARY_HARD, SummaryLength, commit_blocked, commit_label, summary_length};
use glowfly_model::state::repo_identity::{commit_identity_line, env_tip};
use glowfly_model::state::{AppState, ViewState};

/// Height the commit box needs below the file lists (the lists share what is left), with its
/// caption taking `lines` lines.
pub(super) fn reserved_height(lines: usize) -> f32 {
    340.0 + reason::LINE_H * lines as f32
}
/// Between the summary's text and its length, inside the field.
const COUNTER_GAP: f32 = 8.0;
/// Between the two options.
const OPTION_GAP: f32 = 16.0;
/// Between the title and the identity.
const TITLE_GAP: f32 = 8.0;

/// The commit button's tooltip: the keys that commit from the message fields.
pub(super) fn shortcuts_tip(t: Lang, keys: &Keymap) -> String {
    match (keys.hint(BindingId::Commit), keys.hint(BindingId::StageAllAndCommit)) {
        (Some(c), Some(a)) => t.trf("commit.shortcuts.both", &[("commit", &c), ("all", &a)]),
        (Some(c), None) => t.trf("commit.shortcuts.commit_only", &[("commit", &c)]),
        (None, Some(a)) => t.trf("commit.shortcuts.all_only", &[("all", &a)]),
        (None, None) => t.tr("commit.shortcuts.none").to_owned(),
    }
}

/// How the commit box is drawn: on its own under the file lists (`titled`, with its Commit
/// title over a rule), or inside step 3 of an operation in progress, whose title it takes; and
/// whether its button is the view's primary (not beside an operation's Continue).
#[derive(Clone, Copy)]
pub(super) struct BoxStyle {
    pub titled: bool,
    pub primary: bool,
}

pub(super) fn show(ui: &mut egui::Ui, st: &AppState, view: &mut ViewState, cache: &mut UiCache, style: BoxStyle, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    // The fields are drawn from a copy, sent back as one edit when it changed.
    let mut f = st.session.draft.fields();
    // The commit message pane: from here to the bottom of the panel (`pane_focus.rs`).
    let pane = egui::Rect::from_min_max(ui.cursor().min, ui.max_rect().max);
    super::pane_focus::drawn_at(ui.ctx(), glowfly_model::state::layout::Pane::Message, pane);
    let t = st.settings.lang();
    if style.titled {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
    }
    title_row(ui, st, snap, t, style);
    issue_link_row(ui, st, snap, &f, t, msgs);

    let spell = st.settings.spellcheck;
    let s = summary_field(ui, &mut f.summary, cache, t, spell);
    if std::mem::take(&mut view.focus_message) {
        s.request_focus();
    }
    let d = spell_text_edit(ui, &mut f.description, t.tr("commit.description_hint"), 4, &mut cache.speller, spell);
    super::access::name_field(&d, t.tr("commit.description_name"));
    if let Some(template) = &st.session.draft.template {
        let note = t.trf("commit.template_note", &[("char", &template.comment_char)]);
        ui.label(RichText::new(note).size(type_scale::SMALL).color(color::faint()));
    }

    // Off while a merge, cherry-pick or revert is stopped; still on (so it can be turned off)
    // when it was checked before the operation stopped.
    let amend_why = amend_blocked_in(t, snap);
    // Halfway through a rebase there is nothing to push: the option isn't offered.
    let rebasing = rebase_stopped(snap);
    let detached = snap.head_branch.is_none();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = OPTION_GAP;
        let enabled = amend_why.is_none() || f.amend;
        let amend = ui.add_enabled(enabled, controls::Check::new(&mut f.amend, t.tr("commit.amend")));
        let amend = match amend_why {
            Some(why) => amend.on_disabled_hover_text(why),
            None => amend,
        };
        if amend.changed() && f.amend {
            msgs.push(Msg::Commit(CommitMsg::PrefillAmend));
        }
        if rebasing {
            return;
        }
        let mut push = st.settings.push_after_commit;
        let r = ui
            .add_enabled(!detached, controls::Check::new(&mut push, t.tr("commit.push_after.check")))
            .on_hover_text(t.tr("commit.push_after.tip"))
            .on_disabled_hover_text(t.tr("commit.push_after.detached"));
        // The setting changes through update, which also saves it.
        if r.changed() {
            msgs.push(Msg::Commit(CommitMsg::SetPushAfterCommit(push)));
        }
    });

    let merging = snap.state == git2::RepositoryState::Merge;
    // At a rebase stopped to edit, the button commits and continues the rebase.
    let continues = edit_stop(snap).is_some();
    let caption = commit_blocked(st, false);
    let label = commit_label(st);
    let actions = if continues { Vec::new() } else { menu_actions(st, f.amend, amend_why, caption.as_deref()) };
    let tip = shortcuts_tip(t, &st.keymap);
    let b = button::Button { label, enabled: caption.is_none(), primary: style.primary, caption: caption.as_deref(), tip: &tip };
    let (commit, picked) = ui.horizontal(|ui| button::show(ui, b, &actions, &mut f, t, msgs)).inner;
    if let Some(caption) = &caption {
        super::access::describe(&commit, caption);
    }
    if commit.clicked() {
        // A merge commits through Commit merge, which fills a blank message with git's.
        msgs.push(Msg::Commit(if merging {
            CommitMsg::CommitMerge
        } else if continues {
            CommitMsg::CommitAndContinue
        } else {
            CommitMsg::Commit
        }));
    }
    if picked == Some(Act::Amend) {
        f.amend = true;
        view.focus_message = true;
        msgs.push(Msg::Commit(CommitMsg::PrefillAmend));
    }
    // Wrapped, never cut: a long caption (German, a narrow panel) takes a second line.
    let lines = reason::lines(ui, caption.as_deref(), ui.available_width());
    reason::wrapped(ui, caption.as_deref(), lines);

    if s.has_focus() || d.has_focus() {
        let (all, one) = ui.input_mut(|i| {
            // Shift first: consume_key ignores an extra Shift, so Ctrl+Enter would match both.
            let all = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter);
            (all, !all && i.consume_key(Modifiers::COMMAND, Key::Enter))
        });
        if all {
            msgs.push(Msg::Commit(CommitMsg::StageAllAndCommit));
        } else if one {
            msgs.push(Msg::Commit(if continues { CommitMsg::CommitAndContinue } else { CommitMsg::Commit }));
        }
    }
    if let Some(tab) = st.active_tab_id().filter(|_| f != st.session.draft.fields()) {
        msgs.push(Msg::Edit(EditMsg::Commit { tab, fields: f }));
    }
}

/// The header: the "Commit" title when the box is `titled`, then the author identity git gives
/// (in the warning tone, with the variables on hover, when the environment sets it).
fn title_row(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, t: Lang, style: BoxStyle) {
    let identity = st.identities.get(&snap.git_dir).map(|id| (commit_identity_line(t, id, &st.settings), env_tip(t, id)));
    if style.titled || identity.is_some() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TITLE_GAP;
            if style.titled {
                ui.label(type_scale::section_text(ui, t.tr("commit.title")));
            }
            // It reads on from the title ("Commit as Ada"), cut to the room left, smaller than
            // the title so the row keeps its height.
            if let Some(((line, warn), tip)) = identity {
                let tone = if warn { color::warn() } else { color::faint() };
                let max_w = ui.available_width();
                let label = truncate_label(ui, RichText::new(line).size(type_scale::SMALL).color(tone), max_w);
                if let Some(tip) = tip {
                    label.on_hover_text(tip);
                }
            }
        });
    }
}

/// The issue named by the branch, as a chip, with a button that links it from the description
/// until the message mentions it.
fn issue_link_row(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, f: &CommitFields, t: Lang, msgs: &mut Vec<Msg>) {
    // Link the issue encoded in the branch name (e.g. 1234-login or PROJ-12-crash).
    if let Some((fs, key)) = st.session.forge.as_ref().and_then(|f| Some((f, f.branch_issue(snap.head_branch.as_deref()?)?))) {
        let tag = fs.mention(&key);
        let mentioned = fs.links_issue(&f.summary, &key) || fs.links_issue(&f.description, &key);
        ui.horizontal(|ui| {
            issue_chip(ui, fs, &key, msgs);
            let hover = if fs.message_ref(&key) == tag { t.tr("commit.link_tip.mention") } else { t.tr("commit.link_tip.link") };
            if !mentioned
                && ui
                    .add(buttons::secondary(t.trf("commit.link_button", &[("tag", &tag)])).size(ControlSize::Small))
                    .on_hover_text(hover)
                    .clicked()
            {
                msgs.push(Msg::Commit(CommitMsg::AppendToDescription(fs.message_ref(&key))));
            }
        });
    }
}

/// The summary field, with its length inside its trailing edge from the first character, in a
/// room kept for three digits so the text never runs under it.
fn summary_field(ui: &mut egui::Ui, summary: &mut String, cache: &mut UiCache, t: Lang, spell: bool) -> egui::Response {
    let font = type_scale::font(type_scale::SMALL);
    let counter = |s: &str| format!("{}/{SUMMARY_HARD}", s.chars().count());
    let width = |ui: &egui::Ui, s: &str| ui.painter().layout_no_wrap(s.to_owned(), font.clone(), color::weak()).size().x;
    let reserve = width(ui, &counter(summary)).max(width(ui, &format!("000/{SUMMARY_HARD}")));
    ui.scope(|ui| {
        let trailing = Some(reserve + COUNTER_GAP + FIELD_PAD_X);
        let s = spell_text_edit_trailing(ui, summary, t.tr("commit.summary_hint"), trailing, &mut cache.speller, spell);
        super::access::name_field(&s, t.tr("commit.summary_name"));
        // Counted after the edit, so the number keeps up with typing.
        let len = summary.chars().count();
        if len > 0 {
            let tone = match summary_length(len) {
                SummaryLength::Fine => color::faint(),
                SummaryLength::Long => color::weak(),
                SummaryLength::TooLong => color::warn(),
            };
            // As tall as the field, so the count centres on it.
            let at = egui::pos2(s.rect.right() - FIELD_PAD_X - reserve, s.rect.top());
            let slot = egui::Rect::from_min_size(at, vec2(reserve, s.rect.height()));
            ui.scope_builder(egui::UiBuilder::new().max_rect(slot).layout(Layout::right_to_left(Align::Center)), |ui| {
                let text = RichText::new(counter(summary)).font(font.clone()).color(tone);
                ui.label(text).on_hover_text(t.tr("commit.summary_count.tip"));
            });
        }
        s
    })
    .inner
}

/// The caret menu's actions (none at a rebase stopped to edit, where only Skip hooks is
/// offered): Stage all and commit, Commit and push (unless the button already pushes, or a
/// rebase is stopped), Amend last commit (unless Amend is ticked).
fn menu_actions(st: &AppState, amend: bool, amend_why: Option<&str>, caption: Option<&str>) -> Vec<MenuAction> {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return Vec::new() };
    let mut actions = vec![MenuAction {
        label: t.tr("commit.menu.stage_all").to_owned(),
        key: st.keymap.hint(BindingId::StageAllAndCommit).unwrap_or_default().to_owned(),
        why: commit_blocked(st, true),
        act: Act::StageAllAndCommit,
    }];
    if !st.settings.push_after_commit && !rebase_stopped(snap) {
        let why = if snap.head_branch.is_none() { Some(t.tr("commit.push_after.detached")) } else { caption };
        let why = why.map(str::to_owned);
        actions.push(MenuAction { label: t.tr("commit.menu.commit_push").to_owned(), key: String::new(), why, act: Act::CommitAndPush });
    }
    if !amend {
        let why = amend_why.map(str::to_owned);
        actions.push(MenuAction { label: t.tr("commit.amend").to_owned(), key: String::new(), why, act: Act::Amend });
    }
    actions
}
