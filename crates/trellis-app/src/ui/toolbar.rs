//! Top toolbar: repository and branch pickers, git actions, and app buttons.

use super::common::{caret_after, menu_item, short};
use super::theme::color;
use super::widgets::{self, Icon};
use crate::keymap::{self, BindingId};
use crate::msg::BranchMsg;
use crate::msg::CloneMsg;
use crate::msg::ForgeMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::PublishMsg;
use crate::msg::RepoMsg;
use crate::state::{AppState, PrefPage};
use crate::terminal::TerminalMsg;
use eframe::egui::{self, Align, Layout, RichText};
use std::path::PathBuf;
use trellis_core::forge::Provider;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let labels = st.settings.toolbar_labels;
    let tool = |ui: &mut egui::Ui, icon, label, enabled| widgets::tool_button(ui, icon, label, enabled, labels);
    egui::TopBottomPanel::top("toolbar")
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(10, 4)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(48.0);
                // repository ▾ › branch ▾
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new("repository").size(11.0).color(color::weak()));
                    let name = st.snap.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| "Open…".into());
                    let r =
                        ui.menu_button(RichText::new(name).size(16.0).strong().color(color::text()), |ui| repo_menu(ui, &st.recent, msgs));
                    caret_after(ui, &r.response);
                });
                let Some(snap) = &st.snap else { return };
                ui.add_space(12.0);
                ui.label(RichText::new("›").size(20.0).color(color::weak()));
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new("branch").size(11.0).color(color::weak()));
                    let head = snap
                        .head_branch
                        .clone()
                        .unwrap_or_else(|| snap.head_oid.map(|o| format!("detached {}", short(o))).unwrap_or_default());
                    let r = ui.menu_button(RichText::new(head).size(16.0).color(color::text()), |ui| {
                        ui.set_min_width(220.0);
                        for b in &snap.locals {
                            let text = if b.is_head { format!("✔ {}", b.name) } else { b.name.clone() };
                            if ui.add_enabled(!b.is_head, egui::Button::new(text)).clicked() {
                                msgs.push(Msg::Job(JobMsg::Git {
                                    label: "Checkout".into(),
                                    cmds: vec![crate::state::branches::checkout_local(&b.name)],
                                }));
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        menu_item(msgs, ui, "New branch…", Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
                    });
                    caret_after(ui, &r.response);
                });

                ui.add_space(52.0);
                let idle = st.job.is_none();
                if tool(ui, Icon::Undo, "Undo", idle && !st.undo.undo.is_empty()).on_hover_text(st.undo.undo_tooltip()).clicked() {
                    msgs.push(Msg::Job(JobMsg::Undo));
                }
                if tool(ui, Icon::Redo, "Redo", idle && !st.undo.redo.is_empty()).on_hover_text(st.undo.redo_tooltip()).clicked() {
                    msgs.push(Msg::Job(JobMsg::Redo));
                }
                ui.add_space(6.0);
                if tool(ui, Icon::Pull, "Pull", idle).on_hover_text("Fast-forward if possible").clicked() {
                    msgs.push(Msg::git("Pull", &["pull", "--ff"]));
                }
                dropdown(ui, "pull-menu", 240.0, |ui| {
                    for (label, a) in [
                        ("Fetch all", &["fetch", "--all", "--prune"][..]),
                        ("Pull (fast-forward if possible)", &["pull", "--ff"][..]),
                        ("Pull (fast-forward only)", &["pull", "--ff-only"][..]),
                        ("Pull (rebase)", &["pull", "--rebase"][..]),
                    ] {
                        if ui.add_enabled(idle, egui::Button::new(label)).clicked() {
                            msgs.push(Msg::git(label, a));
                        }
                    }
                });
                if tool(ui, Icon::Push, "Push", idle).clicked() {
                    msgs.push(Msg::Job(JobMsg::Push { force: false }));
                }
                dropdown(ui, "push-menu", 200.0, |ui| {
                    if ui.add_enabled(idle, egui::Button::new("Force push (with lease)")).clicked() {
                        msgs.push(Msg::confirm(
                            "Force push",
                            "Overwrite the remote branch with your local history?".into(),
                            "Force push",
                            Msg::Job(JobMsg::Push { force: true }),
                        ));
                    }
                    if snap.remotes.is_empty() && ui.add_enabled(idle, egui::Button::new("Publish to GitHub…")).clicked() {
                        msgs.push(Msg::Publish(PublishMsg::PublishDialog));
                    }
                });
                if tool(ui, Icon::Branch, "Branch", idle).clicked() {
                    msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
                }
                if tool(ui, Icon::Stash, "Stash", idle && snap.is_dirty()).clicked() {
                    msgs.push(Msg::git("Stash", &["stash", "push", "-u"]));
                }
                if tool(ui, Icon::Pop, "Pop", idle && !snap.stashes.is_empty()).clicked() {
                    msgs.push(Msg::git("Pop stash", &["stash", "pop"]));
                }
                if let Some(fs) = st.forge.as_ref().filter(|f| f.host.is_some()) {
                    let can_pr = idle && snap.head_branch.is_some();
                    let tip = format!(
                        "Create a {} on {} for this branch",
                        fs.pr_noun(),
                        fs.host_provider().map(Provider::name).unwrap_or_default()
                    );
                    if tool(ui, Icon::PullRequest, "PR", can_pr).on_hover_text(tip).clicked()
                        && let Some(b) = &snap.head_branch
                    {
                        msgs.push(Msg::Forge(ForgeMsg::CreatePrDialog(b.clone())));
                    }
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                terminal_button(ui, st.terminal.open, labels, msgs);
                if tool(ui, Icon::Folder, "Folder", true).clicked() {
                    msgs.push(Msg::OpenFolder(snap.workdir.clone()));
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if tool(ui, Icon::Gear, "Settings", true)
                        .on_hover_text(format!("Preferences ({})", keymap::hint(BindingId::Prefs)))
                        .clicked()
                    {
                        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
                    }
                    if tool(ui, Icon::Search, "Search", true).on_hover_text(keymap::hint(BindingId::Search)).clicked() {
                        st.view.show_search = !st.view.show_search;
                    }
                    if tool(ui, Icon::Activity, "Activity", true).clicked() {
                        st.view.show_log = !st.view.show_log;
                    }
                    if tool(ui, Icon::Refresh, "Refresh", true).on_hover_text("F5").clicked() {
                        msgs.push(Msg::Repo(RepoMsg::Refresh));
                    }
                });
            });
        });
}

/// Shows or hides the terminal panel; right-click opens an external terminal instead. A bar
/// under the button marks the panel as open.
fn terminal_button(ui: &mut egui::Ui, open: bool, labels: bool, msgs: &mut Vec<Msg>) {
    let verb = if open { "Hide" } else { "Show" };
    let tip = format!("{verb} the terminal panel (Ctrl+`)\nRight-click to open an external terminal");
    let resp = widgets::tool_button(ui, Icon::Terminal, "Terminal", true, labels).on_hover_text(tip);
    if open {
        let r = resp.rect;
        let bar = egui::Rect::from_min_max(egui::pos2(r.left() + 6.0, r.bottom() - 2.0), egui::pos2(r.right() - 6.0, r.bottom()));
        ui.painter().rect_filled(bar, 1.0, color::accent());
    }
    if resp.clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::Toggle));
    }
    resp.context_menu(|ui| menu_item(msgs, ui, "Open in external terminal", Msg::OpenTerminal));
}

/// The small ▾ button next to a tool button, opening a menu below it.
fn dropdown(ui: &mut egui::Ui, id: &str, width: f32, contents: impl FnOnce(&mut egui::Ui)) {
    let caret = widgets::caret_button(ui);
    let popup = ui.make_persistent_id(id);
    if caret.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    egui::popup::popup_below_widget(ui, popup, &caret, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(width);
        contents(ui);
    });
}

fn repo_menu(ui: &mut egui::Ui, recent: &[PathBuf], msgs: &mut Vec<Msg>) {
    ui.set_min_width(260.0);
    menu_item(msgs, ui, "Open…", Msg::Repo(RepoMsg::PickRepo));
    menu_item(msgs, ui, "Clone…", Msg::Clone(CloneMsg::Dialog));
    menu_item(msgs, ui, "Init…", Msg::Repo(RepoMsg::InitRepo));
    if !recent.is_empty() {
        ui.separator();
        ui.label(RichText::new("Recent").color(color::weak()));
        for p in recent {
            menu_item(msgs, ui, p.display().to_string(), Msg::Repo(RepoMsg::Open(p.clone())));
        }
    }
}
