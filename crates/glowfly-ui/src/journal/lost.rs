//! The Lost commits tab: while `git fsck` runs, its failure (with Try again) or an empty result
//! fill the body; otherwise the discarded changes Glowfly kept under their own heading, then the
//! lost commits (headed too when both show), newest first, as 56 px painted rows beside the
//! selected one's preview (`lost_preview.rs`). Line 1 is the label (a kept file's path) and the
//! time; line 2 a chip saying what was thrown away ("Dropped stash", "Discarded hunk"), then the
//! short SHA and the author for a commit or stash. Hovering shows the full
//! message; right-click offers the commit's actions and Copy > SHA. A footer says when the list
//! was cut or fsck reported problems.

use super::{chip, lost_preview, split};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::short;
use crate::state::journal_lost::{LostView, apply_blocked, chip as lost_chip, footer_lines, label, restore_file_blocked, restore_plan};
use crate::ui::common::{menu_header, menu_item, menu_item_if};
use crate::ui::empty_state::{EmptyIcon, empty_state};
use crate::ui::pane_status::{self, Failure};
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::menu::separator;
use crate::ui::widgets::row_menu::RowMenuExt;
use crate::ui::widgets::{paint_cut_text, row_fill};
use eframe::egui::{self, Align2, CursorIcon, Rect, Sense, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::journal::{LostCommit, LostKind};
use glowfly_core::repo::Snapshot;
use glowfly_model::journal::{JournalMsg, LostMsg};

const ROW_H: f32 = 56.0;
const PAD_X: f32 = 16.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let Some(view) = st.session.journal.view.as_ref().and_then(|v| v.lost.as_ref()) else {
        return pane_status::loading(ui, t.tr("journal.lost.loading"));
    };
    match &view.result {
        None => pane_status::loading(ui, t.tr("journal.lost.loading")),
        Some(Err(e)) => {
            let retry = Some(Msg::Journal(JournalMsg::Refresh));
            pane_status::failed(ui, t, Failure { title: t.tr("journal.lost.failed"), error: e, retry }, msgs);
        }
        Some(Ok(_)) if view.is_empty() => {
            empty_state(ui, EmptyIcon::Journal, t.tr("journal.lost.empty.title"), t.tr("journal.lost.empty.hint"), &[]);
        }
        Some(Ok(_)) => split(ui, msgs, |ui, msgs| list(ui, st, view, msgs), |ui, msgs| lost_preview::show(ui, st, view, msgs)),
    }
}

/// One row of the list.
enum Line<'a> {
    Heading(&'static str),
    Entry(&'a LostCommit),
    Footer,
}

/// The rows: the kept discards under their heading, then the lost commits (headed when the
/// discards show too), then the footer, which takes the place of one more row.
fn lines<'a>(t: Lang, view: &'a LostView, footer: bool) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    let discarded = view.discarded();
    if !discarded.is_empty() {
        out.push(Line::Heading(t.tr("journal.lost.discarded.heading")));
        out.extend(discarded.iter().map(Line::Entry));
        if !view.commits().is_empty() {
            out.push(Line::Heading(t.tr("journal.tab.lost.label")));
        }
    }
    out.extend(view.commits().iter().map(Line::Entry));
    if footer {
        out.push(Line::Footer);
    }
    out
}

fn list(ui: &mut Ui, st: &AppState, view: &LostView, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    let footer = footer_lines(t, view);
    let rows = lines(t, view, !footer.is_empty());
    egui::ScrollArea::vertical().id_salt("journal-lost-rows").auto_shrink([false, false]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for i in range {
            let c = match &rows[i] {
                Line::Entry(c) => *c,
                Line::Heading(text) => {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
                    let at = pos2(rect.left() + PAD_X, rect.bottom() - 14.0);
                    ui.painter().text(at, Align2::LEFT_CENTER, *text, type_scale::section(ui), color::weak());
                    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
                    continue;
                }
                Line::Footer => {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
                    let font = type_scale::font(type_scale::SMALL);
                    let top = rect.center().y - 9.0 * (footer.len() as f32 - 1.0);
                    for (k, line) in footer.iter().enumerate() {
                        let at = pos2(rect.center().x, top + 18.0 * k as f32);
                        ui.painter().text(at, Align2::CENTER_CENTER, *line, font.clone(), color::weak());
                    }
                    continue;
                }
            };
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
            let name = label(t, c);
            crate::ui::access::name_button(&resp, true, &name);
            if ui.is_rect_visible(rect) {
                row_fill::paint(ui.painter(), rect, view.selected == Some(c.oid), resp.hovered());
                paint(ui, st, rect, c, &name);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let resp = resp.on_hover_text(glowfly_core::text::printable(c.message.trim_end()));
            if resp.clicked() {
                msgs.push(Msg::Journal(JournalMsg::Lost(LostMsg::Select(c.oid))));
            }
            resp.row_menu(|ui| menu(ui, st, snap, c, &name, msgs));
            ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
        }
    });
}

/// Paints one lost commit's two lines.
fn paint(ui: &Ui, st: &AppState, rect: Rect, c: &LostCommit, name: &str) {
    let t = st.settings.lang();
    let p = ui.painter();
    let (left, right) = (rect.left() + PAD_X, rect.right() - PAD_X);
    let (y1, y2) = (rect.top() + 18.0, rect.top() + 39.0);
    let small = type_scale::font(type_scale::SMALL);
    let when = st.settings.format_time(c.time, st.view.now, st.view.utc_offset);
    let time = p.text(pos2(right, y1), Align2::RIGHT_CENTER, when, small.clone(), color::weak());
    let body = type_scale::font(type_scale::BODY);
    paint_cut_text(p, pos2(left, y1), name, body, color::text(), (time.left() - 12.0 - left).max(0.0));

    let mut x = left;
    if let Some(what) = lost_chip(t, &c.kind) {
        x += chip(p, x, y2, what, (right - x) * 0.5, color::text()).width() + 6.0;
    }
    if matches!(c.kind, LostKind::Discarded { .. }) {
        return;
    }
    let mono = type_scale::mono(type_scale::SMALL);
    let sha = paint_cut_text(p, pos2(x, y2), &short(c.oid), mono, color::weak(), (right - x).max(0.0));
    x += sha.rect.width() + 8.0;
    paint_cut_text(p, pos2(x, y2), &c.author, small, color::weak(), (right - x).max(0.0));
}

/// The row menu: the label as its header, the commit's actions, then Copy > SHA.
fn menu(ui: &mut Ui, st: &AppState, snap: &Snapshot, c: &LostCommit, name: &str, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let lost = |m: LostMsg| Msg::Journal(JournalMsg::Lost(m));
    menu_header(ui, name);
    match c.kind {
        LostKind::Commit => {
            menu_item(msgs, ui, t.tr("journal.create_branch"), lost(LostMsg::CreateBranch(c.oid)));
            let why = restore_plan(snap, c, st.job.is_some(), t).err();
            menu_item_if(msgs, ui, t.tr("journal.restore.button"), why.as_deref(), lost(LostMsg::Restore(c.oid)));
        }
        LostKind::Stash { .. } => {
            let why = apply_blocked(snap, st.job.is_some(), t);
            menu_item_if(msgs, ui, t.tr("journal.lost.apply"), why, lost(LostMsg::Apply(c.oid)));
            menu_item(msgs, ui, t.tr("journal.lost.store"), lost(LostMsg::Store(c.oid)));
        }
        LostKind::Discarded { .. } => {
            let why = restore_file_blocked(st.job.is_some(), t);
            menu_item_if(msgs, ui, t.tr("journal.lost.restore_file"), why, lost(LostMsg::RestoreFile { id: c.oid, copy: false }));
            menu_item_if(msgs, ui, t.tr("journal.lost.restore_copy"), why, lost(LostMsg::RestoreFile { id: c.oid, copy: true }));
            return;
        }
    }
    separator(ui);
    menu_item(msgs, ui, t.tr("commits.menu.copy_sha_item"), Msg::Copy(c.oid.to_string()));
}
