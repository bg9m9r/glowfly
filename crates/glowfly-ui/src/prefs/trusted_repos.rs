//! Preferences > Trusted repositories: what the page is for, a filter once the list is long
//! (`trust_list::FILTER_FROM`) and Forget missing folders once the folder check found any, then
//! every saved decision (trusted_repos_row.rs) or why there is none to show. The page edits
//! only its filter; revoking and forgetting are `TrustMsg`s for update/trust_revoke.rs.

use super::{intro, trusted_repos_row};
use crate::theme::sizes::ControlSize;
use crate::theme::tokens::radius;
use crate::theme::{color, controls, type_scale};
use crate::widgets::buttons;
use eframe::egui::{self, Align, CornerRadius, Id, Layout, RichText, Stroke, TextEdit, Ui};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::trust_list::{FILTER_FROM, TrustRow, missing_paths, shown};

/// The widest the list gets, so a row's button stays near its name.
const MAX_W: f32 = 760.0;

/// What the page shows: every decision, with when it was made as the user reads dates.
pub struct TrustedView {
    pub rows: Vec<TrustRow>,
    /// `rows`' dates ("Decided …" text), in the same order; None where none was kept.
    pub when: Vec<Option<String>>,
    /// The folder check reported (no marks and no Forget button while it runs).
    pub checked: bool,
    pub t: Lang,
}

/// `filter` is the frame's copy of the filter field.
pub fn trusted_page(ui: &mut Ui, v: &TrustedView, filter: &mut String, msgs: &mut Vec<Msg>) {
    let t = v.t;
    ui.set_max_width(MAX_W);
    intro(ui, t.tr("prefs.trust.intro.help"));
    if controls_line(ui, v, filter, msgs) {
        ui.add_space(10.0);
    }
    // The list's heading, which the Preferences search reveals.
    let heading = RichText::new(t.tr("prefs.trust.decisions.label")).font(type_scale::strong(ui, type_scale::SMALL)).color(color::weak());
    ui.label(heading).on_hover_text(t.tr("prefs.trust.decisions.help"));
    ui.add_space(4.0);
    if v.rows.is_empty() {
        ui.label(RichText::new(t.tr("prefs.trust.empty")).color(color::weak()));
        return;
    }
    let listed: Vec<(&TrustRow, Option<&str>)> = v
        .rows
        .iter()
        .zip(&v.when)
        .filter(|(r, _)| !shown(std::slice::from_ref(*r), filter).is_empty())
        .map(|(r, w)| (r, w.as_deref()))
        .collect();
    if listed.is_empty() {
        ui.label(RichText::new(t.tr("prefs.trust.no_match")).color(color::weak()));
        return;
    }
    egui::Frame::new()
        .fill(color::panel())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(CornerRadius::same(radius::LG))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, (row, when)) in listed.into_iter().enumerate() {
                if i > 0 {
                    let x = ui.max_rect().x_range();
                    let y = ui.cursor().top();
                    ui.painter().hline(x, y, Stroke::new(1.0_f32, color::border()));
                }
                trusted_repos_row::row(ui, t, row, when, v.checked, msgs);
            }
        });
}

/// The filter field (from `FILTER_FROM` decisions on, or while it holds text) and, at the
/// right, Forget missing folders once the check found any. Returns whether it drew anything.
fn controls_line(ui: &mut Ui, v: &TrustedView, filter: &mut String, msgs: &mut Vec<Msg>) -> bool {
    let t = v.t;
    let filtering = v.rows.len() >= FILTER_FROM || !filter.is_empty();
    let missing = if v.checked { missing_paths(&v.rows).len() } else { 0 };
    if !filtering && missing == 0 {
        return false;
    }
    ui.horizontal(|ui| {
        if filtering {
            let field = TextEdit::singleline(filter).id(Id::new("glowfly-trust-filter")).hint_text(t.tr("prefs.trust.filter"));
            let field = ui.add(controls::SizedField::new(ControlSize::Regular, field).width(super::FIELD_W));
            crate::access::name_if_unnamed(&field, t.tr("prefs.trust.filter"));
        }
        if missing > 0 {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let text = t.trn("prefs.trust.forget_missing.button", missing, &[]);
                let tip = format!("{}: {}", t.tr("prefs.trust.forget_missing.label"), t.tr("prefs.trust.forget_missing.help"));
                if ui.add(buttons::secondary(text).tip(&tip)).clicked() {
                    msgs.push(Msg::Trust(TrustMsg::ForgetMissing));
                }
            });
        }
    });
    true
}
