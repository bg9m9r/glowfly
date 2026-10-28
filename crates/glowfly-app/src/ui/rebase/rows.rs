//! The list of the rebase panel: the locked rows of a running rebase above it, one row per step
//! (commit rows in `commit_row`, exec and break rows and the row menu in `step_row`, label,
//! reset and merge marks and update-ref rows in `merge_row`), the inline
//! editor under its row (`editor`), dragging rows (`drag`), and the panel's keys (`keys`).

use super::UiCache;
use super::commit_row::{RowKind, paint_action, paint_row};
use super::step_row::paint_step;
use super::{commit_row, drag, editor, keys, merge_row, step_row};
use crate::msg::{Msg, RebaseMsg};
use crate::state::AppState;
use crate::state::rebase::RebasePanel;
use crate::state::rebase_remaining::LOCKED_NOTE;
use crate::state::tabs::index_after_move;
use crate::ui::theme::color;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Color32, CursorIcon, Rect, Response, RichText, Sense, Ui, pos2, vec2};
use glowfly_core::rebase::{RebaseAction, ResetTo, TodoStep};
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::Settings;

pub(super) const ROW_H: f32 = 38.0;
pub(super) const GAP: f32 = 4.0;
pub(super) const MARGIN: f32 = 12.0;
pub(super) const GRIP_X: f32 = 16.0;
pub(super) const COMBO_X: f32 = 32.0;
pub(super) const COMBO_W: f32 = 96.0;
pub(super) const SHA_X: f32 = 140.0;
pub(super) const INDENT: f32 = 22.0;
/// Room for four 26 px buttons at the right of a row.
pub(super) const BUTTONS_W: f32 = 118.0;
pub(super) const META_W: f32 = 230.0;

/// One row's place in the list and its interactions this frame.
pub(super) struct Slot {
    pub rect: Rect,
    pub r: Response,
    /// The row can be dragged (label, reset and merge rows are fixed).
    pub movable: bool,
}

/// What a row is drawn with besides its step.
pub(super) struct Look<'a> {
    pub settings: &'a Settings,
    pub now: i64,
    pub offset: i32,
    /// The loaded history, for the summaries of commits a reset row names.
    pub snap: Option<&'a Snapshot>,
}

/// Row `i` of `n`, and whether the list can be edited.
#[derive(Clone, Copy)]
pub(super) struct Place {
    pub i: usize,
    pub n: usize,
    pub enabled: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Shade {
    Plain,
    Hovered,
    Selected,
    Floating,
}

pub(super) fn fill(shade: Shade) -> Color32 {
    match shade {
        Shade::Plain | Shade::Floating => color::panel(),
        Shade::Hovered => color::hover(),
        Shade::Selected => color::selected(),
    }
}

/// A step as screen readers and tests know its row.
fn row_label(step: &TodoStep) -> String {
    match step {
        TodoStep::Commit(item) => item.summary.clone(),
        TodoStep::Merge(m) => m.item.summary.clone(),
        TodoStep::Exec(cmd) => format!("exec {cmd}"),
        TodoStep::Break => "break".into(),
        TodoStep::UpdateRef(r) => format!("update-ref {r}"),
        TodoStep::Label { name, .. } => format!("label {name}"),
        TodoStep::Reset(to) => match to {
            ResetTo::Onto => "reset onto".into(),
            ResetTo::Label(name) => format!("reset {name}"),
            ResetTo::Commit(o) => format!("reset {}", &o.to_string()[..7]),
            ResetTo::NewRoot => "reset [new root]".into(),
        },
    }
}

/// A row without its controls; `with_action` paints a commit's action in place of its combo
/// box. `text` is a label or reset row's (`RebasePanel::step_text`).
fn paint(ui: &Ui, rect: Rect, step: &TodoStep, text: &str, look: &Look, shade: Shade, with_action: bool) {
    match step {
        TodoStep::Commit(item) | TodoStep::Merge(glowfly_core::rebase::MergeStep { item, .. }) => {
            let merge = step.merge().is_some();
            paint_row(ui, rect, item, look, shade, !merge);
            if merge {
                merge_row::paint_merge_mark(ui, rect, item.action == RebaseAction::Drop);
            }
            if with_action {
                paint_action(ui, rect, item.action);
            }
        }
        TodoStep::Label { .. } => merge_row::paint_label(ui, rect, text, shade),
        TodoStep::Reset(_) => merge_row::paint_reset(ui, rect, text, shade),
        TodoStep::UpdateRef(_) => {
            let branch = step.update_ref().unwrap_or_default();
            let text = look.settings.lang().trf("rebase.update_ref.text", &[("branch", &branch)]);
            merge_row::paint_update_ref(ui, rect, &text, shade);
        }
        step => paint_step(ui, rect, step, shade),
    }
}

pub(super) fn list(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let look = Look { settings: &st.settings, now: st.view.now, offset: st.view.utc_offset, snap: st.snap.as_ref() };
    let spell = st.settings.spellcheck;
    let t = st.settings.lang();
    let covered = st.modal.is_some() || st.palette.is_some() || st.prefs.is_some() || st.askpass.showing();
    let Some(panel) = st.session.rebase.view.as_mut() else { return };
    let enabled = panel.editable();
    let can_drag = enabled && panel.editing.is_none() && panel.items.len() > 1;
    egui::ScrollArea::vertical().id_salt("rebase-list").auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.y = GAP;
        locked(ui, panel, &look);
        let n = panel.items.len();
        let mut slots = Vec::with_capacity(n);
        let list_top = ui.cursor().top();
        let editor_row = panel.editor_row();
        if panel.editing.is_none() {
            editor::forget(ui);
        }
        for i in 0..n {
            let rect = row_rect(ui);
            let step = &panel.items[i];
            let sense = if can_drag { Sense::click_and_drag() } else { Sense::click() };
            let id = match step.oid() {
                Some(oid) => ui.id().with(("rebase-row", oid)),
                None => ui.id().with(("rebase-step", i)),
            };
            let r = ui.interact(rect, id, sense);
            let label = row_label(step);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, enabled, &label));
            let r = if matches!(step, TodoStep::UpdateRef(_)) { r.on_hover_text(t.tr("rebase.update_ref.tip")) } else { r };
            slots.push(Slot { rect, r, movable: panel.movable(i) });
            if editor_row == Some(i) {
                editor::show(ui, t, panel, cache, spell, msgs);
            }
        }
        let floating = if can_drag { drag::drag(ui, &slots, list_top, ROW_H + GAP, msgs) } else { None };
        for (i, Slot { rect, r, movable }) in slots.iter().enumerate() {
            let step = &panel.items[i];
            let text = panel.step_text(i, look.snap);
            match floating {
                Some(f) if f.from == i => {}
                // While dragging, the others slide to where they would be after the drop.
                Some(f) => {
                    let shift = (index_after_move(i, f.from, f.to) as f32 - i as f32) * (ROW_H + GAP);
                    paint(ui, rect.translate(vec2(0.0, shift)), step, &text, &look, Shade::Plain, true);
                }
                None => {
                    let shade = if panel.selected == Some(i) {
                        Shade::Selected
                    } else if r.hovered() {
                        Shade::Hovered
                    } else {
                        Shade::Plain
                    };
                    paint(ui, *rect, step, &text, &look, shade, false);
                    let at = Place { i, n, enabled };
                    match step {
                        TodoStep::Commit(item) => {
                            let kind = RowKind { head: panel.heads_group(i), actions: &RebaseAction::ALL, fixed: false };
                            commit_row::controls(ui, *rect, at, item, kind, msgs)
                        }
                        TodoStep::Merge(m) => {
                            let kind = RowKind { head: false, actions: panel.actions(i), fixed: true };
                            commit_row::controls(ui, *rect, at, &m.item, kind, msgs)
                        }
                        TodoStep::Label { .. } | TodoStep::Reset(_) | TodoStep::UpdateRef(_) => {}
                        _ => step_row::step_controls(ui, *rect, at, step, msgs),
                    }
                    if can_drag && *movable && drag::grip_rect(*rect).contains(ui.input(|inp| inp.pointer.hover_pos()).unwrap_or_default())
                    {
                        ui.ctx().set_cursor_icon(CursorIcon::Grab);
                    }
                    if r.clicked() {
                        msgs.push(Msg::Rebase(RebaseMsg::RebaseSelect(i)));
                    }
                    let on_text = r.interact_pointer_pos().is_some_and(|p| p.x >= rect.left() + SHA_X);
                    let editable = !matches!(step, TodoStep::Break | TodoStep::Label { .. } | TodoStep::Reset(_) | TodoStep::UpdateRef(_));
                    if enabled && r.double_clicked() && on_text && editable {
                        msgs.push(Msg::Rebase(RebaseMsg::RebaseEditMessage(i)));
                    }
                    if enabled {
                        step_row::row_menu(r, i, step, msgs);
                    }
                }
            }
        }
        // The dragged row, drawn last so it floats above the others.
        if let Some(f) = floating {
            paint(ui, f.rect, &panel.items[f.from], &panel.step_text(f.from, look.snap), &look, Shade::Floating, true);
        }
        super::add_buttons(ui, panel, msgs);
        keys::keys(ui, covered, panel, &slots, msgs);
        ui.add_space(12.0);
    });
}

/// The next row's rect, inset by the list's margin.
fn row_rect(ui: &mut Ui) -> Rect {
    let width = ui.available_width() - 2.0 * MARGIN;
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    Rect::from_min_size(pos2(outer.left() + MARGIN, outer.top()), vec2(width, ROW_H))
}

/// The steps finishing a reword whose pick a running rebase already did: dimmed, without
/// controls, under a note.
fn locked(ui: &mut Ui, panel: &RebasePanel, look: &Look) {
    let Some(steps) = panel.remaining.as_ref().map(|r| &r.locked).filter(|l| !l.is_empty()) else { return };
    ui.horizontal(|ui| {
        ui.add_space(MARGIN + 4.0);
        ui.label(RichText::new(LOCKED_NOTE).size(type_scale::SMALL).color(color::weak()));
    });
    for (n, step) in steps.iter().enumerate() {
        let rect = row_rect(ui);
        let label = row_label(step);
        let r = ui.interact(rect, ui.id().with(("rebase-locked", n)), Sense::hover());
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, false, &label));
        paint(ui, rect, step, "", look, Shade::Plain, true);
        // Dimmed under a veil of the background.
        ui.painter().rect_filled(rect, egui::CornerRadius::same(radius::SM), color::bg().gamma_multiply(0.55));
    }
    ui.add_space(6.0);
}
