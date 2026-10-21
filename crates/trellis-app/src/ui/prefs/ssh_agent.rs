//! Preferences > SSH: the section header the page's three sections share (the title, an
//! optional status pill, a Refresh icon button at the right end, and the rule under them), and
//! the Agent section: its pill (`state::ssh_marks::agent_pill`), the Use local SSH agent
//! setting, what the agent holds and why it may not be reachable, and on Windows the Windows'
//! OpenSSH row (`ssh_windows`). The view never edits settings; it sends `Msg`s.

use super::ssh::SshContext;
use super::{GAP, LABEL_W, ROW_W, fit, help, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use crate::state::ssh_marks::{AgentPill, agent_pill};
use crate::ui::theme::{color, controls, type_scale};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::i18n::Lang;
use trellis_core::ssh_keys::{AgentReach, WinService};

/// The pill's height, its dot and the room around them.
const PILL_H: f32 = 22.0;
const DOT: f32 = 7.0;
const PILL_PAD: f32 = 10.0;
const DOT_GAP: f32 = 6.0;

/// A section's header, as `form::section` draws one, with `pill` after the title and a Refresh
/// icon button (tip `refresh_tip`, pushing `msg`) at the right end of the line.
pub(super) fn header(ui: &mut Ui, t: Lang, title: &str, pill: Option<AgentPill>, refresh_tip: &str, msg: Msg, msgs: &mut Vec<Msg>) {
    ui.add_space(6.0);
    let w = fit(ui, LABEL_W + GAP + ROW_W);
    ui.allocate_ui_with_layout(vec2(w, 26.0), Layout::left_to_right(Align::Center), |ui| {
        ui.label(type_scale::section_text(ui, title));
        if let Some(p) = pill {
            ui.add_space(6.0);
            pill_widget(ui, t, p);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Refresh, refresh_tip).clicked() {
                msgs.push(msg);
            }
        });
    });
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0_f32, color::border()));
    ui.add_space(10.0);
}

/// The pill's text, hover tip and colour.
fn pill_parts(t: Lang, p: AgentPill) -> (String, Option<&'static str>, Color32) {
    match p {
        AgentPill::Checking => (t.tr("prefs.ssh.agent.pill.checking").into(), None, color::weak()),
        AgentPill::Running(0) => {
            (t.tr("prefs.ssh.agent.pill.running_empty").into(), Some(t.tr("prefs.ssh.agent.pill.running.tip")), color::added())
        }
        AgentPill::Running(n) => {
            (t.trn("prefs.ssh.agent.pill.running", n, &[]), Some(t.tr("prefs.ssh.agent.pill.running.tip")), color::added())
        }
        AgentPill::Unreachable => {
            (t.tr("prefs.ssh.agent.pill.unreachable").into(), Some(t.tr("prefs.ssh.agent.pill.unreachable.tip")), color::warn())
        }
        AgentPill::Off => (t.tr("prefs.ssh.agent.pill.off").into(), Some(t.tr("prefs.ssh.agent.pill.off.tip")), color::weak()),
    }
}

/// A rounded capsule: a dot in the state's colour, then its text, on a faint fill of it.
fn pill_widget(ui: &mut Ui, t: Lang, p: AgentPill) {
    let (text, tip, tint) = pill_parts(t, p);
    let galley = ui.painter().layout_no_wrap(text.clone(), type_scale::font(type_scale::SMALL), color::text());
    let w = PILL_PAD + DOT + DOT_GAP + galley.size().x + PILL_PAD;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, PILL_H), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same((PILL_H / 2.0) as u8), tint.gamma_multiply(0.18));
    let cy = rect.center().y;
    painter.circle_filled(pos2(rect.left() + PILL_PAD + DOT / 2.0, cy), DOT / 2.0, tint);
    let text_pos = pos2(rect.left() + PILL_PAD + DOT + DOT_GAP, cy - galley.size().y / 2.0);
    painter.galley(text_pos, galley, color::text());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
    if let Some(tip) = tip {
        resp.on_hover_text(tip);
    }
}

/// The Agent section: header with the pill, the setting, then what the check found.
pub(super) fn agent_section(ui: &mut Ui, t: Lang, page: &SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    let pill = agent_pill(page.agent.as_ref());
    let recheck = Msg::Ssh(SshMsg::ReloadAgent);
    header(ui, t, t.tr("prefs.ssh.agent.title"), Some(pill), t.tr("prefs.ssh.agent.recheck.tip"), recheck, msgs);
    ui.vertical(|ui| {
        ui.set_max_width(fit(ui, LABEL_W + GAP + ROW_W));
        let mut on = cx.use_agent;
        if ui.add(controls::Check::new(&mut on, t.tr("prefs.ssh.agent.use.check"))).changed() {
            msgs.push(Msg::Ssh(SshMsg::SetUseAgent(on)));
        }
        help(ui, t.tr("prefs.ssh.agent.use.help"));
        let Some(a) = &page.agent else { return };
        ui.add_space(4.0);
        match &a.reach {
            Some(AgentReach::Keys(keys)) => {
                for k in keys {
                    let line = if k.comment.is_empty() { k.fingerprint.clone() } else { format!("{}  {}", k.fingerprint, k.comment) };
                    ui.label(RichText::new(line).monospace().size(type_scale::SMALL).color(color::faint()));
                }
            }
            Some(AgentReach::Failed(e)) => warn(ui, &t.trf("prefs.ssh.agent.failed.warn", &[("error", e)])),
            Some(AgentReach::NoAgent) | None => {}
        }
        if a.ssh_add.is_none() {
            warn(ui, t.tr("prefs.ssh.agent.no_ssh_add.warn"));
        }
        // The service is only looked up on Windows; without it, this is another system.
        match a.service {
            Some(s) => {
                let line = match s {
                    WinService::Running => t.tr("prefs.ssh.agent.service.running.help"),
                    WinService::Stopped => t.tr("prefs.ssh.agent.service.stopped.help"),
                    WinService::Disabled => t.tr("prefs.ssh.agent.service.disabled.help"),
                    WinService::Missing => t.tr("prefs.ssh.agent.service.missing.help"),
                };
                help(ui, line);
            }
            None if !a.auth_sock_set && !a.running() => warn(ui, t.tr("prefs.ssh.agent.no_auth_sock.warn")),
            None => {}
        }
        super::ssh_windows::windows_row(ui, t, page, msgs);
        if a.pageant {
            help(ui, t.tr("prefs.ssh.agent.pageant.help"));
        }
        if let Some(e) = &page.agent_error {
            warn(ui, e);
        }
    });
}
