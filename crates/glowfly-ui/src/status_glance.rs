//! The status bar's glance items (design pass D15, rules in `state/tabs/glance*.rs`): the active
//! tab's last failure at the left, with no job running ("Fetch failed · Details" in `warn`, and
//! a × that dismisses it), and when the repository was last fetched at the right ("Fetched 4
//! min ago"; its tooltip lists each remote, and a click fetches). That item repaints once a
//! minute, when its text can change, rather than every frame.
//!
//! Like `tab_state.rs`, this is a deliberate exception to the rule that chrome paints no status
//! colours (`vivid_tests`): the failure is painted with `color::warn()` because D15 asks for an
//! outcome the user hasn't seen to stand out until it is dismissed.

use super::status::CONTROL;
use super::theme::{color, type_scale};
use super::widgets::{Icon, buttons, paint_icon_sized};
use eframe::egui::{self, RichText, Sense, vec2};
use glowfly_model::logs::LogsMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::state::tabs::glance_fetch;
use std::time::Duration;

/// The warning triangle's size before the failure's text.
const MARK: f32 = 12.0;

/// The active tab's last failure, unless it was dismissed. Whether it showed.
pub(super) fn failure_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) -> bool {
    let Some(tab) = st.tabs.get(st.active_tab) else { return false };
    let Some(failure) = st.tab_glance(st.active_tab).failed else { return false };
    let t = st.settings.lang();
    let (rect, _) = ui.allocate_exact_size(vec2(MARK, MARK), Sense::hover());
    paint_icon_sized(ui.painter(), rect.center(), Icon::Warning, color::warn(), MARK);
    let text = failure.headline(t);
    ui.label(RichText::new(text).color(color::warn()).font(type_scale::strong(ui, type_scale::BODY)));
    ui.label(RichText::new("·").color(color::faint()));
    let details = buttons::quiet(t.tr("glance.details")).size(CONTROL).tip(t.tr("glance.details.tip"));
    if ui.add(details).clicked() {
        msgs.push(Msg::Logs(LogsMsg::Details { tab: tab.id, entry: Some(failure.entry) }));
    }
    let dismiss = buttons::icon_only(Icon::Close, t.tr("glance.dismiss")).size(CONTROL).tip(t.tr("glance.dismiss"));
    if ui.add(dismiss).clicked() {
        msgs.push(Msg::Logs(LogsMsg::DismissFailure(failure.entry)));
    }
    true
}

/// When the active repository was last fetched; a click fetches every remote, unless a job runs
/// (then disabled with the reason). Not shown without a repository or a remote.
pub(super) fn fetch_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = st.snap.as_ref().filter(|s| !s.remote_info.is_empty()) else { return };
    let t = st.settings.lang();
    let (now, offset) = (st.view.now, st.view.utc_offset);
    let fetching = glance_fetch::fetching(st);
    let text = glance_fetch::label(t, &st.settings, snap.fetched_at, fetching, now, offset);
    if let Some(secs) = glance_fetch::changes_in(snap.fetched_at, now) {
        ui.ctx().request_repaint_after(Duration::from_secs(secs));
    }
    let tip = glance_fetch::tip(t, &st.settings, snap, now, offset);
    let blocked = glowfly_model::state::toolbar_tips::fetch_blocked(st);
    let button = egui::Button::new(RichText::new(text).color(color::weak())).frame(false);
    let r = ui.add_enabled(blocked.is_none(), button).on_hover_text(&tip);
    let r = match &blocked {
        Some(why) => r.on_disabled_hover_text(format!("{tip}\n{why}")),
        None => r,
    };
    if r.clicked() {
        msgs.push(glance_fetch::fetch_all());
    }
}
