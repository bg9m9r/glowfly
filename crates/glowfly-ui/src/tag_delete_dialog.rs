//! The Delete tag dialog (`Modal::DeleteTag`), in the danger confirmation's form: the question,
//! then, when the repository has remotes, a checkbox per remote to delete the tag from too (none
//! ticked at first; a long list scrolls) and a weak note on what undo and other clones keep. The
//! danger button counts the ticked remotes and waits while a job runs; Cancel is focused and is
//! what Enter presses.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::widgets::buttons;
use crate::theme::sizes::ControlSize;
use crate::theme::{color, controls};
use eframe::egui::{self, RichText, Ui};
use glowfly_model::msg::Msg;
use glowfly_model::state::blocked;
use glowfly_model::state::tag_delete::DeleteTagForm;

/// More remotes than this scroll inside the list.
const SHOWN_REMOTES: usize = 6;

/// Draws the dialog; returns true when Cancel, Esc or the backdrop closes it. The delete closes
/// it through `update` once the job starts, so a refusal leaves it open.
pub(super) fn show(ui: &mut Ui, cx: Cx, form: &mut DeleteTagForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let mut d = Dialog::new(cx, t.tr("tag.delete.title")).size(Size::Small).begin(ui);
    d.body(ui, |ui| {
        ui.add(egui::Label::new(t.trf("tag.delete.body", &[("tag", &form.tag)])).wrap());
        if form.remotes.is_empty() {
            return;
        }
        ui.add_space(8.0);
        ui.label(t.tr("tag.delete.remotes_label"));
        let scrolls = form.remotes.len() > SHOWN_REMOTES;
        let mut rows = |ui: &mut Ui| {
            for (name, ticked) in &mut form.remotes {
                controls::checkbox(ui, ticked, name.as_str());
            }
        };
        if scrolls {
            // A checkbox row is a standard control's height.
            let height = ControlSize::Regular.metrics().h * SHOWN_REMOTES as f32;
            egui::ScrollArea::vertical().id_salt("tag-delete-remotes").max_height(height).auto_shrink([false, true]).show(ui, rows);
        } else {
            rows(ui);
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(t.tr("tag.delete.remotes_note")).color(color::weak())).wrap());
    });
    let danger = buttons::danger(form.button(t)).enabled(idle).why((!idle).then_some(blocked::busy(t)));
    let out = d.finish(ui, Footer::new().danger(danger).cancel());
    if out.primary {
        msgs.push(form.delete_msg());
    }
    out.dismiss
}
