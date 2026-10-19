//! The cross-repository dashboard, filling the central area: a header with the last update, a
//! Refresh and a Close button; the Mine / Review requested / Assigned switch and a filter; the
//! rows grouped by service account and repository (`dashboard_rows.rs`); and what couldn't be
//! loaded, with the way to fix it.

use super::dashboard_rows::{issue_row, pr_row, repo_row};
use super::empty_state::{EmptyIcon, PaneButton, empty_state};
use super::theme::color;
use super::widgets::segmented::{Segment, segmented_opts};
use super::widgets::{self, Icon};
use crate::msg::{CloneMsg, DashboardMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::dashboard::{AccountState, DashShow, Phase, service_header};
use crate::state::dashboard_rows::{Row, counts, empty_hint, empty_text, sections};
use crate::state::{AppState, PrefPage};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Layout, Margin, RichText, Ui};
use trellis_core::forge::Account;

pub(super) const NO_MATCH_TITLE: &str = "Nothing matches the filter";
const NO_MATCH_HINT: &str = "The filter looks at each title, number, author and repository.";
pub(super) const CLEAR_FILTER: &str = "Clear filter";

fn dash(m: DashboardMsg) -> Msg {
    Msg::Dashboard(m)
}

/// An icon button with an accessible name, so tests and screen readers find it.
fn icon(ui: &mut Ui, icon: Icon, label: &str) -> egui::Response {
    let r = widgets::icon_button(ui, icon, label);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    r
}

pub(super) fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    egui::Frame::new().inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 4 }).show(ui, |ui| {
        header(ui, st, msgs);
        ui.add_space(10.0);
        let d = &st.dashboard;
        if d.basis.is_none() && !d.is_busy() {
            return no_repositories(ui, st, msgs);
        }
        controls(ui, st, msgs);
        ui.add_space(6.0);
        ui.separator();
        let filter = st.dashboard.filter.trim().to_lowercase();
        let st: &AppState = st;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.spacing_mut().item_spacing.y = 0.0;
            body(ui, st, &filter, msgs);
            not_loaded(ui, st, msgs);
            ui.add_space(12.0);
        });
    });
}

/// No repositories are known yet: open, clone, or connect a service.
fn no_repositories(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let list = [
        PaneButton { text: t.tr("common.open_repo.button"), primary: true },
        PaneButton { text: t.tr("common.clone_repo.button"), primary: false },
        PaneButton { text: t.tr("dashboard.empty.connect.button"), primary: false },
    ];
    match empty_state(ui, EmptyIcon::Repos, t.tr("dashboard.empty.title"), t.tr("dashboard.empty.hint"), &list) {
        Some(0) => msgs.push(Msg::Repo(RepoMsg::PickRepo)),
        Some(1) => msgs.push(Msg::Clone(CloneMsg::Dialog)),
        Some(_) => msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations))),
        None => {}
    }
}

/// The switch lists nothing: nothing matches the filter, or nothing waits.
fn nothing_listed(ui: &mut Ui, show: DashShow, filter: &str, msgs: &mut Vec<Msg>) {
    if filter.is_empty() {
        empty_state(ui, EmptyIcon::Inbox, empty_text(show), empty_hint(show), &[]);
    } else if empty_state(ui, EmptyIcon::Search, NO_MATCH_TITLE, NO_MATCH_HINT, &[PaneButton { text: CLEAR_FILTER, primary: false }])
        .is_some()
    {
        msgs.push(dash(DashboardMsg::ClearFilter));
    }
}

fn header(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let d = &st.dashboard;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("Dashboard").font(type_scale::strong(ui, type_scale::TITLE)).color(color::strong()));
            ui.label(RichText::new("Pull requests and issues across your repositories").color(color::weak()));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon(ui, Icon::Close, st.settings.lang().tr("common.close.button")).clicked() {
                msgs.push(dash(DashboardMsg::Close));
            }
            if icon(ui, Icon::Refresh, "Refresh").clicked() {
                msgs.push(dash(DashboardMsg::Refresh));
            }
            ui.add_space(6.0);
            let asking = d.accounts.iter().filter(|a| a.state == AccountState::Pending).count();
            let status = match (d.phase, d.fetched_at) {
                (Phase::Scanning, _) => "Reading repositories…".to_owned(),
                (Phase::Fetching, _) => format!("Asking {asking} service{}…", if asking == 1 { "" } else { "s" }),
                (Phase::Idle, Some(at)) => format!("Updated {}", trellis_core::dates::relative(st.view.now - at)),
                (Phase::Idle, None) => String::new(),
            };
            let r = ui.label(RichText::new(status).color(color::weak()));
            if let (Phase::Idle, Some(at)) = (d.phase, d.fetched_at) {
                // The label is already relative, so the hover is only the full date.
                r.on_hover_text(st.settings.format_verbose(at, st.view.utc_offset));
            }
        });
    });
}

fn controls(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let n = counts(&st.dashboard, &st.dashboard.filter.trim().to_lowercase());
    // What each segment lists, in DashShow::index order.
    const TIPS: [&str; 3] = [
        "Open pull requests you opened",
        "Open pull requests waiting for your review",
        "Open issues, work items and cards assigned to you",
    ];
    let labels = DashShow::ALL.map(|s| format!("{} ({})", s.label(), n[s.index()]));
    let options = DashShow::ALL.map(|s| Segment::new(s, labels[s.index()].as_str(), TIPS[s.index()]));
    ui.horizontal(|ui| {
        if let Some(s) = segmented_opts(ui, st.dashboard.show, &options) {
            msgs.push(dash(DashboardMsg::Show(s)));
        }
        ui.add_space(12.0);
        controls::field(
            ui,
            egui::TextEdit::singleline(&mut st.dashboard.filter)
                .hint_text("Filter by title, number, author or repository")
                .desired_width(320.0),
        );
    });
}

fn body(ui: &mut Ui, st: &AppState, filter: &str, msgs: &mut Vec<Msg>) {
    let d = &st.dashboard;
    let found = sections(d, d.show, filter);
    let rows: usize = found.iter().flat_map(|s| &s.repos).map(|r| r.rows.len()).sum();
    if rows == 0 && d.phase == Phase::Idle {
        nothing_listed(ui, d.show, filter, msgs);
    }
    for s in &found {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(service_header(s.account)).size(type_scale::CAPTION).color(color::weak()));
        });
        ui.add_space(2.0);
        for repo in &s.repos {
            repo_row(ui, st, repo.data, msgs);
            if let Some(e) = repo.error {
                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    ui.label(RichText::new(e).size(type_scale::SMALL).color(color::red()));
                });
            }
            let checks = repo.rows.iter().any(|r| matches!(r, Row::Pr(p) if p.checks.is_some()));
            let slot = if checks { super::build_status::MARKER_SLOT } else { 0.0 };
            for row in &repo.rows {
                match row {
                    Row::Pr(pr) => pr_row(ui, st, repo.data, pr, slot, msgs),
                    Row::Issue(i) => issue_row(ui, st, repo.data, i, msgs),
                }
            }
        }
    }
}

fn account_name(a: &Account) -> String {
    format!("{} · {}", a.provider.name(), a.display())
}

/// One "Not loaded" line: the text, then its buttons.
fn problem(ui: &mut Ui, text: String, add_buttons: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        ui.label(RichText::new(text).size(type_scale::SMALL).color(color::text()));
        add_buttons(ui);
    });
    ui.add_space(4.0);
}

fn connect(ui: &mut Ui, msgs: &mut Vec<Msg>) {
    if ui.add(buttons::secondary("Connect…").small()).clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
    }
}

fn folder_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| trellis_core::native_path::shown(p))
}

/// Accounts that couldn't be asked, repositories without a service, unreadable folders and
/// targets past the limit.
fn not_loaded(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let d = &st.dashboard;
    let failing = d.accounts.iter().filter(|a| !matches!(a.state, AccountState::Ok | AccountState::Pending) || a.me_error.is_some());
    let any = failing.clone().next().is_some() || !d.no_service.is_empty() || !d.unreadable.is_empty() || d.over_limit > 0;
    if !any {
        return;
    }
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Not loaded").font(type_scale::strong(ui, type_scale::BODY)).color(color::strong()));
    });
    ui.add_space(6.0);
    for a in failing {
        let name = account_name(&a.account);
        match &a.state {
            AccountState::NotConnected => problem(ui, format!("{name}: not connected"), |ui| connect(ui, msgs)),
            AccountState::SignInFailed(e) => problem(ui, format!("{name}: {e}"), |ui| {
                if ui.add(buttons::secondary("Sign in").small()).clicked() {
                    msgs.push(dash(DashboardMsg::SignIn(a.account.clone())));
                }
                connect(ui, msgs);
            }),
            AccountState::Ok | AccountState::Pending => {}
        }
        if let Some(e) = &a.me_error {
            problem(ui, format!("{name}: Couldn't find your account: {e}"), |_| {});
        }
    }
    if !d.no_service.is_empty() {
        let n = d.no_service.len();
        let title = if n == 1 {
            "1 repository has no hosting-service remote".to_owned()
        } else {
            format!("{n} repositories have no hosting-service remote")
        };
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            egui::CollapsingHeader::new(RichText::new(title).size(type_scale::SMALL)).id_salt("dashboard-no-service").show(ui, |ui| {
                for p in &d.no_service {
                    ui.label(RichText::new(folder_name(p)).size(type_scale::SMALL).color(color::weak()))
                        .on_hover_text(trellis_core::native_path::shown(p));
                }
            });
        });
        ui.add_space(4.0);
    }
    for (p, why) in &d.unreadable {
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            ui.label(RichText::new(format!("{}: {why}", folder_name(p))).size(type_scale::SMALL).color(color::text()))
                .on_hover_text(trellis_core::native_path::shown(p));
        });
        ui.add_space(4.0);
    }
    if d.over_limit > 0 {
        let n = d.over_limit;
        let what = if n == 1 { "1 more repository wasn't".to_owned() } else { format!("{n} more repositories weren't") };
        problem(ui, format!("{what} asked (limit {})", crate::state::dashboard::MAX_TARGETS), |_| {});
    }
}
