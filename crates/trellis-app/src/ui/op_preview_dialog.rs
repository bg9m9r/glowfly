//! "Preview: <op>" (`Modal::OpPreview`): what a merge, rebase, reset, cherry-pick or drop would
//! leave, worked out without changing anything. The body is a loading line, the error, or the
//! headline, the history afterwards (`op_preview_rows.rs`), the conflicted paths, the commits
//! nothing will reach and the notes; the footer applies the operation as its menu item would
//! (a reset offers all three modes), and offers Preview again for an error or a stale result.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::branch_colours::line;
use super::theme::{color, type_scale};
use super::widgets::buttons;
use crate::msg::{Msg, OpPreviewMsg, ResetMode};
use crate::state::op_preview::{self as op, PreviewDialog};
use eframe::egui::{self, Color32, RichText, Ui};
use trellis_core::i18n::Lang;
use trellis_core::op_preview::{OpPreview, PreviewOp};
use trellis_core::repo::Snapshot;
use trellis_core::settings::Settings;

/// What the dialog is drawn against, gathered before the modal borrows the state.
pub struct Env<'a> {
    pub snap: Option<&'a Snapshot>,
    pub settings: &'a Settings,
    pub now: i64,
    pub offset: i32,
    /// No job is running.
    pub idle: bool,
}

/// Draws the dialog; returns true when Cancel was pressed.
pub fn show(ui: &mut Ui, cx: Cx, d: &PreviewDialog, env: &Env, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let Some(snap) = env.snap else { return true };
    let title = op::title(t, snap, &d.job.request.op);
    let mut dlg = Dialog::new(cx, title).subtitle(t.tr("op_preview.subtitle")).size(Size::Wide).resizable("op-preview").begin(ui);
    dlg.body(ui, |ui| match &d.result {
        None => super::pane_status::loading_line(ui, t.tr("op_preview.loading")),
        Some(Err(e)) => {
            let text = t.trf("op_preview.failed", &[("error", &trellis_core::error::plain_message(e))]);
            ui.add(egui::Label::new(RichText::new(text).color(color::text())).wrap());
        }
        Some(Ok(p)) => result(ui, t, env, snap, p),
    });
    let why = op::apply_blocker(t, !env.idle, snap, d);
    // A disabled footer shows `why` above its buttons, so a stale preview says so there.
    let again = matches!(d.result, Some(Err(_))) || op::stale(snap, &d.job);
    let mut footer = Footer::new();
    if again {
        footer = footer.extra(buttons::secondary(t.tr("op_preview.again.button")));
    }
    footer = footer.cancel();
    let reset = matches!(d.job.request.op, PreviewOp::Reset { .. });
    let button = |label: &'static str, primary: bool| {
        let b = if primary { buttons::primary(label) } else { buttons::secondary(label) };
        b.enabled(why.is_none()).why(why.as_deref())
    };
    footer = if reset {
        footer
            .alt(button(t.tr("op_preview.apply.reset_soft"), false))
            .alt(button(t.tr("op_preview.apply.reset_hard"), false))
            .primary(button(t.tr("op_preview.apply.reset_mixed"), true))
    } else {
        footer.primary(button(apply_label(t, &d.job.request.op), true))
    };
    let out = dlg.finish(ui, footer);
    if out.primary {
        msgs.push(Msg::OpPreview(OpPreviewMsg::Apply(reset.then_some(ResetMode::Mixed))));
    }
    match out.alt {
        Some(0) => msgs.push(Msg::OpPreview(OpPreviewMsg::Apply(Some(ResetMode::Soft)))),
        Some(1) => msgs.push(Msg::OpPreview(OpPreviewMsg::Apply(Some(ResetMode::Hard)))),
        _ => {}
    }
    if out.extra == Some(0) {
        msgs.push(Msg::OpPreview(OpPreviewMsg::Again));
    }
    out.dismiss
}

fn apply_label(t: Lang, op: &PreviewOp) -> &'static str {
    match op {
        PreviewOp::Merge { .. } => t.tr("op_preview.apply.merge"),
        PreviewOp::Rebase { .. } => t.tr("op_preview.apply.rebase"),
        PreviewOp::CherryPick(_) => t.tr("op_preview.apply.cherry_pick"),
        PreviewOp::Drop(_) => t.tr("op_preview.apply.drop"),
        PreviewOp::Reset { .. } => t.tr("op_preview.apply.reset_mixed"),
    }
}

/// The worked-out preview: headline, history, conflicts, the commits nothing will reach, notes.
fn result(ui: &mut Ui, t: Lang, env: &Env, snap: &Snapshot, p: &OpPreview) {
    ui.spacing_mut().item_spacing.y = 6.0;
    for line in op::headline(t, p) {
        ui.add(egui::Label::new(RichText::new(line).color(color::text())).wrap());
    }
    ui.add_space(10.0);
    ui.label(RichText::new(t.tr("op_preview.after")).font(type_scale::section(ui)).color(color::weak()));
    let tone: Color32 = match p.subject.as_deref() {
        Some(b) => line(snap.branch_swatch(b)),
        None => color::accent(),
    };
    super::op_preview_rows::after(ui, env, p, tone);
    if let Some(paths) = op::conflicts(p) {
        ui.add_space(4.0);
        super::conflict_check::file_list(ui, t, paths, false);
    }
    if p.gone_count.n > 0 {
        ui.add_space(10.0);
        let title = t.trn("op_preview.gone.title", p.gone_count.n, &[]);
        ui.label(RichText::new(title).font(type_scale::section(ui)).color(color::weak()));
        super::op_preview_rows::gone(ui, env, &p.gone);
        let more = p.gone_count.n.saturating_sub(p.gone.len());
        if more > 0 {
            ui.label(RichText::new(t.trn("op_preview.more", more, &[])).color(color::weak()));
        }
        ui.label(RichText::new(t.tr("op_preview.gone.hint")).color(color::weak()));
    }
    let notes = op::notes(t, snap, p);
    if !notes.is_empty() {
        ui.add_space(6.0);
        for n in notes {
            ui.add(egui::Label::new(RichText::new(n).color(color::weak())).wrap());
        }
    }
}
