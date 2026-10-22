//! The "Merge" section: whether the service would merge now, the merge methods the service
//! offers, the commit title and message, deleting the source branch afterwards, and the Merge
//! button (with a confirmation, and why it is disabled under it). A merged pull request offers to delete its source branch.

use super::super::theme::color;
use super::{heading, note, tone_color};
use crate::msg::Msg;
use crate::state::pull_request::{PrAction, PrView};
use crate::state::pull_request_actions::{
    delete_after_merge_blocked, delete_branch_offer, error_for, merge_blocked, merge_button, merge_choices, merge_confirm, mergeability,
    method_blocked, method_help, method_label,
};
use crate::ui::theme::controls;
use crate::ui::widgets::{buttons, reason};
use eframe::egui::{self, RichText, Ui};
use glowfly_core::forge::{MergeMethod, PrState};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_core::text::printable;

/// Shown once the page is loaded.
pub(super) fn show(ui: &mut Ui, t: Lang, v: &mut PrView, snap: Option<&Snapshot>, default_branch: Option<&str>, msgs: &mut Vec<Msg>) {
    let Some(page) = v.loaded() else { return };
    let state = page.detail.state;
    heading(ui, "Merge");
    let target = printable(&v.pr.target);
    match state {
        PrState::Merged => {
            note(ui, format!("Merged into {target}."), color::text());
            let offer = snap.and_then(|s| delete_branch_offer(t, v, s, default_branch));
            if let Some(confirm) = offer
                && ui.add(buttons::secondary(format!("Delete {} on {}", printable(&v.pr.source), v.remote))).clicked()
            {
                msgs.push(confirm);
            }
        }
        PrState::Closed => note(ui, "Closed without merging.", color::weak()),
        PrState::Open => open(ui, v, default_branch, msgs),
    }
}

fn open(ui: &mut Ui, v: &mut PrView, default_branch: Option<&str>, msgs: &mut Vec<Msg>) {
    if let Some((tone, text)) = mergeability(v) {
        note(ui, text, tone_color(tone));
        ui.add_space(4.0);
    }
    let Some(detail) = v.loaded().map(|p| p.detail.clone()) else { return };
    let target = printable(&v.pr.target);
    for m in merge_choices(&detail) {
        let blocked = method_blocked(&detail, m);
        let mut r = ui.add_enabled(blocked.is_none(), controls::Radio::new(v.forms.merge_method == Some(m), method_label(m)));
        if let Some(why) = blocked {
            r = r.on_disabled_hover_text(why);
        }
        if r.clicked() {
            v.forms.merge_method = Some(m);
        }
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            note(ui, method_help(m, &target), color::weak());
        });
    }
    if matches!(v.forms.merge_method, Some(MergeMethod::Merge | MergeMethod::Squash)) {
        ui.add_space(4.0);
        ui.label(RichText::new("Commit title").color(color::text()));
        let title =
            egui::TextEdit::singleline(&mut v.forms.merge_title).hint_text("Empty: the service's default").desired_width(f32::INFINITY);
        controls::field(ui, title);
        ui.label(RichText::new("Commit message").color(color::text()));
        let message = egui::TextEdit::multiline(&mut v.forms.merge_message)
            .hint_text("Empty: the service's default")
            .desired_rows(3)
            .desired_width(f32::INFINITY);
        controls::field(ui, message);
    }
    ui.add_space(4.0);
    let source = printable(&v.pr.source);
    if detail.deletes_merged_branches {
        note(ui, "The repository deletes merged branches itself", color::weak());
    } else {
        let blocked = delete_after_merge_blocked(v, default_branch);
        if blocked.is_some() {
            v.forms.delete_branch = false;
        }
        let label = format!("Delete {source} on {} after merging", v.remote);
        let r = ui.add_enabled(blocked.is_none(), controls::Check::new(&mut v.forms.delete_branch, label));
        if let Some(why) = &blocked {
            r.on_disabled_hover_text(why);
        }
    }
    ui.add_space(4.0);
    let blocked = merge_blocked(v);
    let r = ui.add(buttons::secondary(merge_button(v)).enabled(blocked.is_none()).why(blocked.as_deref()));
    if r.clicked() {
        msgs.push(merge_confirm(v, default_branch));
    }
    // Unless the mergeability note above already says it.
    let shown = mergeability(v).map(|(_, text)| text);
    if let Some(why) = blocked.as_deref().filter(|why| shown.as_deref() != Some(*why)) {
        reason::line(ui, Some(why), egui::Align::Min);
    }
    if let Some(e) = error_for(v, &PrAction::Merge) {
        note(ui, e.to_owned(), color::removed());
    }
}
