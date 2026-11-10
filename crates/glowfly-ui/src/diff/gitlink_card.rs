//! The card a diff shows for a submodule's gitlink (`state::gitlink_card`): a painted direction
//! mark and the title, the old and new ids with a painted arrow (each full id on hover), why
//! the commits can't be listed, what a submodule is, a warning for changes inside, the commits
//! the move passes over (oldest first, with a line for the earlier ones not listed), and Open
//! submodule and Record this checkout. It replaces the two "Subproject commit" rows; above a
//! type change's rows it takes at most part of the height. In the LFS summary's frame, so the
//! two read as one family.

use crate::theme::{color, type_scale};
use crate::widgets::icon;
use eframe::egui::{self, Label, Painter, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::state::gitlink_card::{self, CardText, Mark};
use glowfly_model::submodule::SubmoduleMsg;

/// The share of the diff's height the card may take above a type change's rows.
const ABOVE_ROWS: f32 = 0.45;
/// The marks' line width.
const STROKE: f32 = 1.5;

/// Draws the card when the open diff is a gitlink's; true when it replaces the diff's rows.
pub(super) fn shown(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) -> bool {
    let Some(m) = gitlink_card::shown(st) else { return false };
    let t = st.settings.lang();
    let card = gitlink_card::text(t, m);
    let (record, open) = (gitlink_card::record_offered(st), gitlink_card::open_offered(st));
    let max = if m.type_change { ui.available_height() * ABOVE_ROWS } else { f32::INFINITY };
    show(ui, t, &card, record.as_deref(), open.as_deref(), max, msgs);
    !m.type_change
}

/// The card, scrolling within `max` points of height.
pub(super) fn show(ui: &mut Ui, t: Lang, card: &CardText, record: Option<&str>, open: Option<&str>, max: f32, msgs: &mut Vec<Msg>) {
    ui.add_space(6.0);
    let scroll = egui::ScrollArea::vertical().id_salt("gitlink_card").auto_shrink([false, true]).max_height(max);
    scroll.show(ui, |ui| {
        super::lfs_note::card_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            contents(ui, t, card, record, open, msgs);
        });
        ui.add_space(6.0);
    });
}

fn contents(ui: &mut Ui, t: Lang, card: &CardText, record: Option<&str>, open: Option<&str>, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        if let Some(mark) = card.mark {
            let (r, _) = ui.allocate_exact_size(vec2(icon::SIZE_MD, icon::SIZE_MD), Sense::hover());
            paint_mark(ui.painter(), r.center(), mark, color::text());
        }
        let title = RichText::new(&card.title).font(type_scale::strong(ui, type_scale::BODY)).color(color::text());
        ui.add(Label::new(title).wrap());
    });
    if let Some((old, new, full_old, full_new)) = &card.ids {
        ui.horizontal(|ui| {
            ui.add(Label::new(mono(old, color::weak()))).on_hover_text(full_old);
            let (r, _) = ui.allocate_exact_size(vec2(24.0, icon::SIZE_SM), Sense::hover());
            paint_arrow(ui.painter(), r, true, color::weak());
            ui.add(Label::new(mono(new, color::text()))).on_hover_text(full_new);
        });
    }
    if let Some(reason) = &card.reason {
        ui.add(Label::new(RichText::new(reason).color(color::text())).wrap());
    }
    ui.add(Label::new(RichText::new(&card.explain).color(color::weak())).wrap());
    if let Some(w) = &card.warning {
        ui.add(Label::new(RichText::new(w).color(color::warn())).wrap());
    }
    for g in &card.groups {
        ui.add_space(4.0);
        ui.label(RichText::new(&g.heading).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        if let Some(more) = &g.more {
            ui.label(RichText::new(more).color(color::weak()));
        }
        for (short, subject, full) in &g.rows {
            ui.horizontal(|ui| {
                ui.add(Label::new(mono(short, color::weak()))).on_hover_text(full);
                ui.add(Label::new(RichText::new(subject).color(color::text())).truncate()).on_hover_text(full);
            });
        }
    }
    if let Some(base) = &card.base {
        ui.add(Label::new(RichText::new(base).color(color::weak())).wrap());
    }
    if open.is_none() && record.is_none() {
        return;
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if let Some(p) = open
            && ui.button(t.tr("diff.gitlink.open.button")).clicked()
        {
            msgs.push(Msg::Submodule(SubmoduleMsg::Open(p.to_owned())));
        }
        if let Some(p) = record
            && ui.button(t.tr("diff.gitlink.record.button")).on_hover_text(t.tr("sidebar.submodules.record.tip")).clicked()
        {
            msgs.push(Msg::Submodule(SubmoduleMsg::Record(p.to_owned())));
        }
    });
}

fn mono(text: &str, col: egui::Color32) -> RichText {
    RichText::new(text).font(type_scale::mono(type_scale::BODY)).color(col)
}

/// An arrow across `r`'s middle, pointing right (`forward`) or left.
fn paint_arrow(p: &Painter, r: Rect, forward: bool, col: egui::Color32) {
    let s = Stroke::new(STROKE, col);
    let (from, to) = if forward { (r.left() + 3.0, r.right() - 3.0) } else { (r.right() - 3.0, r.left() + 3.0) };
    let y = r.center().y;
    p.line_segment([pos2(from, y), pos2(to, y)], s);
    let back = if forward { -4.0 } else { 4.0 };
    p.line_segment([pos2(to, y), pos2(to + back, y - 4.0)], s);
    p.line_segment([pos2(to, y), pos2(to + back, y + 4.0)], s);
}

/// The title's direction mark, 16 points across, centred on `c`: an arrow forward or back, or a
/// line that forks in two for another line of history.
fn paint_mark(p: &Painter, c: Pos2, mark: Mark, col: egui::Color32) {
    let r = Rect::from_center_size(c, vec2(icon::SIZE_MD, icon::SIZE_MD));
    match mark {
        Mark::Forward => paint_arrow(p, r, true, col),
        Mark::Backward => paint_arrow(p, r, false, col),
        Mark::Fork => {
            let s = Stroke::new(STROKE, col);
            let split = c + vec2(-1.0, 0.0);
            p.line_segment([c + vec2(-7.0, 0.0), split], s);
            for end in [c + vec2(6.0, -5.0), c + vec2(6.0, 5.0)] {
                p.line_segment([split, end], s);
                p.circle_filled(end, 1.75, col);
            }
        }
    }
}
