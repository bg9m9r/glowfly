//! The Create issue dialog (`Modal::CreateIssue`): the Jira project, the type, the title (Jira:
//! summary), the description, labels and assignees (`picker`), the Jira epic, and whether to
//! create a branch afterwards. The choices come from the service once loaded; the checks are
//! listed live above the buttons. The view edits the form's text and picks in place; a
//! committed project, Create and Try again are sent as messages.

use super::super::theme::color;
use super::picker::{Picker, multi};
use crate::msg::{IssueMsg, Msg};
use crate::state::issue_form::{IssueForm, OptionsState};
use crate::state::{ForgeState, ProjectKeys};
use eframe::egui::{self, Key, RichText, Ui};

/// Draws the dialog; returns true when it should close. The title takes the keyboard focus
/// when nothing has it, unless `focus` is false (a credential prompt is above it).
pub fn show(ui: &mut Ui, form: &mut IssueForm, fs: Option<&ForgeState>, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(form.heading());
    ui.label(RichText::new(format!("{} · {}", form.provider().name(), form.target.display())).color(color::weak()));
    ui.add_space(8.0);
    ui.spacing_mut().item_spacing.y = 4.0;
    if form.provider().is_jira() {
        caption(ui, "Project");
        project(ui, form, fs, msgs);
    }
    if let Some(types) = form.loaded().map(|o| o.types.clone()).filter(|t| !t.is_empty()) {
        caption(ui, "Type");
        let shown = form.kind_title().unwrap_or("Choose a type").to_owned();
        egui::ComboBox::from_id_salt("issue-type").selected_text(shown).width(200.0).show_ui(ui, |ui| {
            for t in &types {
                ui.selectable_value(&mut form.kind, Some(t.key.clone()), t.title.as_str());
            }
        });
    }
    caption(ui, form.title_label());
    let r = ui.add(egui::TextEdit::singleline(&mut form.title).desired_width(f32::INFINITY));
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus && ui.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
    caption(ui, "Description");
    let d = egui::TextEdit::multiline(&mut form.description).hint_text("Describe the issue (optional)").desired_rows(6);
    ui.add(d.desired_width(f32::INFINITY));
    let opts = form.loaded().cloned().unwrap_or_default();
    caption(ui, "Labels");
    let labels: Vec<(String, String)> = opts.labels.iter().map(|l| (l.clone(), l.clone())).collect();
    let picker = Picker { id_salt: "issue-labels", hint: "Search labels", options: &labels, max: usize::MAX, free: opts.labels_free };
    multi(ui, &picker, &mut form.label_query, &mut form.labels);
    caption(ui, "Assignees");
    let people: Vec<(String, String)> = opts.people.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    let max = if form.loaded().is_some() { opts.max_assignees } else { usize::MAX };
    let picker = Picker { id_salt: "issue-people", hint: "Search people", options: &people, max, free: false };
    multi(ui, &picker, &mut form.people_query, &mut form.people);
    if form.epic_shown() {
        caption(ui, "Epic");
        let shown = match &form.epic {
            Some(k) => opts.epics.iter().find(|e| &e.key == k).map_or_else(|| k.clone(), |e| format!("{}  {}", e.key, e.title)),
            None => "None".to_owned(),
        };
        egui::ComboBox::from_id_salt("issue-epic").selected_text(shown).width(300.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut form.epic, None, "None");
            for e in &opts.epics {
                ui.selectable_value(&mut form.epic, Some(e.key.clone()), format!("{}  {}", e.key, e.title));
            }
        });
    }
    ui.add_space(4.0);
    ui.checkbox(&mut form.then_branch, "Create a branch for it afterwards");
    match &form.options {
        OptionsState::Loading => {
            ui.label(RichText::new("Loading labels, people and types…").size(11.5).color(color::weak()));
        }
        OptionsState::Failed(e) => {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("Couldn't load the choices: {e}")).size(11.5).color(color::faint()));
                if ui.link("Try again").clicked() {
                    msgs.push(Msg::Issue(IssueMsg::SetProject(form.project.clone())));
                }
            });
        }
        OptionsState::NotAsked | OptionsState::Loaded(_) => {}
    }
    let problems = form.problems();
    if !problems.is_empty() || form.error.is_some() {
        ui.add_space(6.0);
    }
    for p in &problems {
        ui.label(RichText::new(p).color(color::red()));
    }
    if let Some(e) = &form.error {
        ui.add(egui::Label::new(RichText::new(format!("Couldn't create the issue: {e}")).color(color::red())).wrap());
    }
    ui.add_space(10.0);
    let blocked = form.submit_blocked();
    ui.horizontal(|ui| {
        let label = if form.busy { "Creating…" } else { "Create issue" };
        let ok = blocked.is_none() && !form.busy;
        let mut r = ui.add_enabled(ok, egui::Button::new(label));
        if let Some(why) = &blocked {
            r = r.on_disabled_hover_text(why);
        }
        if r.clicked() || (submit && ok) {
            msgs.push(Msg::Issue(IssueMsg::Create));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

fn caption(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).size(12.0).color(color::weak()));
}

/// The project: a list of the site's project keys when known, else a field committed on
/// Enter or when it loses focus.
fn project(ui: &mut Ui, form: &mut IssueForm, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let known = fs.and_then(|f| match &f.jira_projects {
        ProjectKeys::Known(set) if !set.is_empty() => {
            let mut keys: Vec<String> = set.iter().cloned().collect();
            keys.sort();
            Some(keys)
        }
        _ => None,
    });
    match known {
        Some(keys) => {
            let shown = if form.project.is_empty() { "Choose a project".to_owned() } else { form.project.clone() };
            egui::ComboBox::from_id_salt("issue-project").selected_text(shown).width(200.0).show_ui(ui, |ui| {
                for k in &keys {
                    if ui.selectable_label(&form.project == k, k.as_str()).clicked() && &form.project != k {
                        msgs.push(Msg::Issue(IssueMsg::SetProject(k.clone())));
                    }
                }
            });
        }
        None => {
            let r = ui.add(egui::TextEdit::singleline(&mut form.project_text).hint_text("Project key, e.g. PROJ").desired_width(200.0));
            if r.lost_focus() && form.project_text.trim() != form.project {
                msgs.push(Msg::Issue(IssueMsg::SetProject(form.project_text.trim().to_owned())));
            }
        }
    }
}
