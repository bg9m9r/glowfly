//! Dialogs: confirmation, new branch, new tag (drawn by `tag_dialog.rs`), clone, create pull
//! request, publish (drawn by `publish.rs`), renaming a branch or setting its upstream (drawn by
//! `branch_dialogs.rs`), creating or renaming a working-tree file (drawn by `file_dialogs.rs`),
//! starting a Git Flow branch (drawn by `gitflow_dialog.rs`) and the keyboard shortcut sheet
//! (drawn by `shortcuts/sheet.rs`).

use super::common::{capitalize, issue_chip, parse_keys, repo_name_from_url};
use super::theme::color;
use crate::msg::{FolderPurpose, Msg};
use crate::state::{AppState, ForgeState, Modal};
use eframe::egui::{self, Key, RichText};
use std::path::{Path, PathBuf};
use trellis_core::forge::{self, NewPullRequest, RefStyle};
use trellis_core::gitcli::args;
use trellis_core::repo::Snapshot;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    // Under a credential prompt, Esc and clicks belong to the prompt.
    let prompting = st.askpass.showing();
    let Some(modal) = st.modal.as_mut() else { return };
    let snap = st.snap.as_ref();
    let fs = st.forge.as_ref();
    let idle = st.job.is_none();
    let stage_renames = st.settings.stage_renames;
    let push_tags = st.settings.push_new_tags;
    let mut close = false;
    let resp = egui::Modal::new(egui::Id::new("trellis-modal")).show(ctx, |ui| {
        ui.set_width(460.0);
        match modal {
            Modal::Confirm { title, body, button, .. } => {
                ui.heading(title.as_str());
                ui.add_space(6.0);
                ui.label(body.as_str());
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(button.as_str()).clicked() {
                        msgs.push(Msg::ConfirmAccepted);
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Modal::NewBranch { start, name, checkout } => {
                let heading = match (start.as_deref(), snap.and_then(|s| s.head_branch.as_deref())) {
                    (Some(s), _) => format!("New branch at {}", &s[..7.min(s.len())]),
                    (None, Some(b)) => format!("New branch from {b}"),
                    (None, None) => "New branch at HEAD".to_owned(),
                };
                ui.heading(heading);
                ui.add_space(6.0);
                let r = ui.add(egui::TextEdit::singleline(name).hint_text("branch-name").desired_width(f32::INFINITY));
                // Not under a credential prompt: its fields need the keyboard.
                if !prompting {
                    r.request_focus();
                }
                ui.checkbox(checkout, "Check out after creating");
                ui.add_space(8.0);
                let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.horizontal(|ui| {
                    let valid = !name.trim().is_empty() && !name.contains(' ');
                    if ui.add_enabled(valid, egui::Button::new("Create")).clicked() || (submit && valid) {
                        let mut cmd = if *checkout { args(&["checkout", "-b", name.trim()]) } else { args(&["branch", name.trim()]) };
                        if let Some(s) = start {
                            cmd.push(s.clone());
                        }
                        msgs.push(Msg::Git { label: "Create branch".into(), cmds: vec![cmd] });
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Modal::NewTag(form) => {
                if super::tag_dialog::new_tag(ui, form, snap, push_tags, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::Clone { url, parent } => {
                ui.heading("Clone a repository");
                ui.add_space(6.0);
                ui.add(egui::TextEdit::singleline(url).hint_text("https://github.com/owner/repo.git").desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(parent).hint_text("Clone into folder").desired_width(340.0));
                    if ui.button("Browse…").clicked() {
                        msgs.push(Msg::PickFolder(FolderPurpose::CloneParent));
                    }
                });
                let name = repo_name_from_url(url);
                if !name.is_empty() && !parent.is_empty() {
                    ui.label(RichText::new(format!("→ {}", Path::new(parent.as_str()).join(&name).display())).color(color::weak()));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let valid = !name.is_empty() && Path::new(parent.as_str()).is_dir();
                    if ui.add_enabled(valid, egui::Button::new("Clone")).clicked() {
                        msgs.push(Msg::CloneRepo {
                            url: url.trim().to_owned(),
                            parent: PathBuf::from(parent.as_str()),
                            name: name.clone(),
                        });
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Modal::CreatePr { source, target, targets, title, description, draft, items } => {
                let Some(fs) = fs.filter(|f| f.host.is_some()) else {
                    close = true;
                    return;
                };
                let form = PrForm { source, target, targets, title, description, draft, items };
                if create_pr(ui, form, fs, snap, idle, msgs) {
                    close = true;
                }
            }
            Modal::Publish(f) => {
                if super::publish::form(ui, f, idle, msgs) {
                    close = true;
                }
            }
            Modal::RenameBranch { old, name } => {
                if super::branch_dialogs::rename(ui, old, name, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::SetUpstream { branch, remote, name } => {
                if super::branch_dialogs::upstream(ui, branch, remote, name, snap, msgs) {
                    close = true;
                }
            }
            Modal::NewFile(form) => {
                if super::file_dialogs::new_file(ui, form, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::RenameFile { old, form } => {
                if super::file_dialogs::rename_file(ui, old, form, snap, stage_renames, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::GitFlowStart(form) => {
                if super::gitflow_dialog::start(ui, form, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::Shortcuts { filter } => {
                if super::shortcuts::sheet(ui, filter, !prompting) {
                    close = true;
                }
            }
        }
    });
    if close || (!prompting && resp.should_close()) {
        msgs.push(Msg::CloseModal);
    }
}

struct PrForm<'a> {
    source: &'a mut String,
    target: &'a mut String,
    targets: &'a mut Vec<String>,
    title: &'a mut String,
    description: &'a mut String,
    draft: &'a mut bool,
    items: &'a mut String,
}

/// The create pull/merge request form; returns true when the dialog should close.
fn create_pr(ui: &mut egui::Ui, f: PrForm, fs: &ForgeState, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let Some(host) = fs.host.as_ref() else { return true };
    let mut close = false;
    let noun = fs.pr_noun();
    ui.heading(format!("Create {noun}"));
    ui.label(RichText::new(format!("{} · {}", host.provider.name(), host.display())).color(color::weak()));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(f.source.as_str()).strong());
        ui.label("into");
        egui::ComboBox::from_id_salt("pr-target").selected_text(f.target.as_str()).width(200.0).show_ui(ui, |ui| {
            for t in f.targets.iter() {
                ui.selectable_value(f.target, t.clone(), t);
            }
        });
    });
    let pushed = snap.is_some_and(|s| s.remotes.iter().any(|(r, b)| *r == fs.remote && b.iter().any(|(n, _)| n == f.source)));
    if !pushed {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} isn't on {} yet.", f.source, fs.remote)).color(color::amber()));
            if ui.add_enabled(idle, egui::Button::new("Push branch")).clicked() {
                msgs.push(Msg::git("Push", &["push", "-u", &fs.remote, f.source]));
            }
        });
    }
    ui.add_space(6.0);
    ui.add(egui::TextEdit::singleline(f.title).hint_text("Title").desired_width(f32::INFINITY));
    ui.add(egui::TextEdit::multiline(f.description).hint_text("Description (Markdown)").desired_rows(5).desired_width(f32::INFINITY));
    let keys = parse_keys(f.items);
    if fs.tracker.is_some() {
        ui.horizontal(|ui| {
            ui.label(format!("{}s", capitalize(fs.issue_noun())));
            let hint = if fs.style() == Some(RefStyle::Jira) { "e.g. PROJ-12, PROJ-14" } else { "e.g. 123, 456" };
            ui.add(egui::TextEdit::singleline(f.items).hint_text(hint).desired_width(220.0));
        });
        for k in &keys {
            issue_chip(ui, fs, k, msgs);
        }
    }
    ui.checkbox(f.draft, "Create as draft");
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let valid = pushed && !f.title.trim().is_empty() && !f.target.is_empty() && f.target != f.source;
        if ui.add_enabled(valid, egui::Button::new(RichText::new(format!("Create {noun}")).strong())).clicked() {
            // Trackers linked by mention get "Closes #12"-style lines; Azure Boards items are
            // linked through the API.
            let mut body = f.description.trim().to_owned();
            let links: Vec<String> = fs
                .tracker
                .as_ref()
                .map(|t| keys.iter().filter_map(|k| forge::link_line(host.provider, t.provider, k)).collect())
                .unwrap_or_default();
            if !links.is_empty() {
                if !body.is_empty() {
                    body.push_str("\n\n");
                }
                body.push_str(&links.join("\n"));
            }
            msgs.push(Msg::CreatePr(NewPullRequest {
                source: f.source.clone(),
                target: f.target.clone(),
                title: f.title.trim().to_owned(),
                description: body,
                draft: *f.draft,
                issues: keys.clone(),
            }));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
