//! The selected lost commit's preview: its label, "author · sha · date", what keeps it (or
//! that a stash was dropped), up to 8 first-parent commits painted as a lane in the weak colour
//! (from the commit, or from the commit a stash was made on), and its actions: Create branch
//! here… and Restore… for a commit, Apply stash and Put back in stash list for a stash. A kept
//! discarded file shows what was thrown away and when, with Restore and Restore as a copy.

use super::preview::lane;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::short;
use crate::state::journal_lost::{LostView, apply_blocked, chip, label, restore_file_blocked, restore_plan};
use crate::ui::pane_status;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::{buttons, reason};
use eframe::egui::{self, Align, RichText, Ui};
use glowfly_core::journal::{LostCommit, LostKind};
use glowfly_model::journal::{JournalMsg, LostMsg};

const INSET: f32 = 16.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, view: &LostView, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let Some(c) = view.selected_commit() else {
        ui.add_space(32.0);
        ui.vertical_centered(|ui| ui.label(RichText::new(t.tr("journal.lost.select_hint")).color(color::weak())));
        return;
    };
    egui::ScrollArea::vertical().id_salt("journal-lost-preview").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(INSET as i8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            pane(ui, st, view, c, msgs);
        });
    });
}

/// A kept discarded file: its path, what was thrown away and when, and its two restores.
fn discarded(ui: &mut Ui, st: &AppState, c: &LostCommit, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.spacing_mut().item_spacing.y = 6.0;
    let title = RichText::new(label(t, c)).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text());
    ui.add(egui::Label::new(title).wrap());
    let date = st.settings.format_verbose(c.time, st.view.utc_offset);
    let meta = format!("{} · {date}", chip(t, &c.kind).unwrap_or_default());
    ui.label(RichText::new(meta).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
    ui.add_space(4.0);
    ui.add(egui::Label::new(RichText::new(t.tr("journal.lost.preview.discarded")).color(color::text())).wrap());
    ui.add_space(12.0);
    let why = restore_file_blocked(st.job.is_some(), t);
    let restore = |copy| Msg::Journal(JournalMsg::Lost(LostMsg::RestoreFile { id: c.oid, copy }));
    ui.horizontal(|ui| {
        if ui.add(buttons::primary(t.tr("journal.lost.restore_file")).enabled(why.is_none()).why(why)).clicked() {
            msgs.push(restore(false));
        }
        if ui.add(buttons::secondary(t.tr("journal.lost.restore_copy")).enabled(why.is_none()).why(why)).clicked() {
            msgs.push(restore(true));
        }
    });
    reason::line(ui, why, Align::Min);
}

fn pane(ui: &mut Ui, st: &AppState, view: &LostView, c: &LostCommit, msgs: &mut Vec<Msg>) {
    if matches!(c.kind, LostKind::Discarded { .. }) {
        return discarded(ui, st, c, msgs);
    }
    let t = st.settings.lang();
    let stash = matches!(c.kind, LostKind::Stash { .. });
    ui.spacing_mut().item_spacing.y = 6.0;
    let title = RichText::new(label(t, c)).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text());
    ui.add(egui::Label::new(title).wrap());
    let date = st.settings.format_verbose(c.time, st.view.utc_offset);
    let meta = format!("{} · {} · {date}", c.author, short(c.oid));
    ui.label(RichText::new(meta).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
    ui.add_space(4.0);
    let about = if stash { t.tr("journal.lost.preview.stash") } else { t.tr("journal.lost.preview.commit") };
    ui.add(egui::Label::new(RichText::new(about).color(color::text())).wrap());
    ui.add_space(10.0);
    let heading = if stash { t.tr("journal.lost.preview.stash_base") } else { t.tr("journal.lost.preview.history") };
    ui.label(RichText::new(heading).font(type_scale::section(ui)).color(color::weak()));
    match view.preview.as_ref().filter(|p| p.oid == c.oid).and_then(|p| p.result.as_ref()) {
        None => pane_status::loading_line(ui, t.tr("journal.preview.loading")),
        Some(Err(e)) => {
            ui.label(RichText::new(crate::ui::common::capitalize(glowfly_core::error::plain_message(e).trim())).color(color::weak()));
        }
        Some(Ok(p)) => lane(ui, st, color::weak(), &p.history),
    }
    ui.add_space(12.0);
    let lost = |m: LostMsg| Msg::Journal(JournalMsg::Lost(m));
    if stash {
        let why = st.snap.as_ref().and_then(|snap| apply_blocked(snap, st.job.is_some(), t));
        ui.horizontal(|ui| {
            if ui.add(buttons::primary(t.tr("journal.lost.apply")).enabled(why.is_none()).why(why)).clicked() {
                msgs.push(lost(LostMsg::Apply(c.oid)));
            }
            if ui.add(buttons::secondary(t.tr("journal.lost.store"))).clicked() {
                msgs.push(lost(LostMsg::Store(c.oid)));
            }
        });
        reason::line(ui, why, Align::Min);
        return;
    }
    let why = st.snap.as_ref().and_then(|snap| restore_plan(snap, c, st.job.is_some(), t).err());
    ui.horizontal(|ui| {
        if ui.add(buttons::primary(t.tr("journal.create_branch"))).clicked() {
            msgs.push(lost(LostMsg::CreateBranch(c.oid)));
        }
        if ui.add(buttons::secondary(t.tr("journal.restore.button")).enabled(why.is_none()).why(why.as_deref())).clicked() {
            msgs.push(lost(LostMsg::Restore(c.oid)));
        }
    });
    reason::line(ui, why.as_deref(), Align::Min);
}
