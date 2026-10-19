//! The Create issue dialog (`Modal::CreateIssue`): the Jira project or Trello board, the type
//! (Bitbucket: kind; Trello: list), the priority (Bitbucket), the title (Jira: summary; Trello:
//! name), the description, labels and assignees (`picker`), the Jira epic or Azure parent, and
//! whether to create a branch afterwards, all captioned in the tracker's words
//! (`issue_words`). The choices come from the service once loaded; the checks are listed live
//! above the buttons. The view edits the form's text and picks in place; a committed project
//! or board, Create, Try again and the write-token link are sent as messages.

use super::super::dialog_frame::{Cx, Dialog, Footer};
use super::super::theme::color;
use super::super::widgets::buttons;
use super::picker::{Picker, multi};
use crate::msg::{IssueMsg, Msg};
use crate::state::issue_form::{IssueForm, OptionsState};
use crate::state::issue_words as words;
use crate::state::{ForgeState, ProjectKeys};
use crate::ui::theme::controls;
use eframe::egui::{self, Key, RichText, Ui};
use trellis_core::forge::Provider;
use trellis_core::forge::filters::Named;
use trellis_core::text::printable;

/// Draws the dialog; returns true when it should close. The title takes the keyboard focus
/// when nothing has it, unless `cx.keyboard` is false (a credential prompt is above it).
pub fn show(ui: &mut Ui, cx: Cx, form: &mut IssueForm, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) -> bool {
    let p = form.provider();
    let subtitle = format!("{} · {}", p.name(), form.target.display());
    let mut d = Dialog::new(cx, form.heading()).subtitle(subtitle).wide().begin(ui);
    if p == Provider::Trello {
        write_note(ui, form, msgs);
        ui.add_space(4.0);
    }
    let submit = d.body(ui, |ui| body(ui, form, fs, cx.keyboard, msgs));
    let blocked = form.submit_blocked();
    let label = if form.busy { "Creating…".to_owned() } else { form.heading() };
    let ok = blocked.is_none() && !form.busy;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary(label).enabled(ok).why(blocked.as_deref())).submit(submit));
    if out.primary {
        msgs.push(Msg::Issue(IssueMsg::Create));
    }
    out.dismiss
}

/// The fields, the loading state and the problems; returns whether Enter left the title.
fn body(ui: &mut Ui, form: &mut IssueForm, fs: Option<&ForgeState>, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let p = form.provider();
    let noun = words::noun(p);
    ui.spacing_mut().item_spacing.y = 4.0;
    if p.is_jira() {
        caption(ui, words::project_label(p));
        project(ui, form, fs, msgs);
    }
    if p == Provider::Trello {
        board(ui, form, msgs);
    }
    if let Some(types) = form.loaded().map(|o| o.types.clone()).filter(|t| !t.is_empty()) {
        caption(ui, words::type_label(p));
        let shown = form.kind_title().unwrap_or("Choose a type").to_owned();
        combo(ui, "issue-type", shown, 200.0, &types, &mut form.kind);
    }
    if let Some(priorities) = form.loaded().map(|o| o.priorities.clone()).filter(|t| !t.is_empty()) {
        caption(ui, words::priority_label(p));
        let chosen = form.priority.as_ref().and_then(|k| priorities.iter().find(|n| &n.key == k));
        let shown = chosen.map_or_else(|| words::priority_hint(p).to_owned(), |n| n.title.clone());
        combo(ui, "issue-priority", shown, 200.0, &priorities, &mut form.priority);
    }
    caption(ui, form.title_label());
    let r = controls::field(ui, egui::TextEdit::singleline(&mut form.title).desired_width(f32::INFINITY));
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus && ui.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
    caption(ui, "Description");
    let d = egui::TextEdit::multiline(&mut form.description).hint_text(format!("Describe the {noun} (optional)")).desired_rows(6);
    controls::field(ui, d.desired_width(f32::INFINITY));
    let opts = form.loaded().cloned().unwrap_or_default();
    // Bitbucket issues have no labels.
    if form.loaded().is_none() || !opts.labels.is_empty() || opts.labels_free {
        caption(ui, words::labels_label(p));
        let labels: Vec<(String, String)> = opts.labels.iter().map(|l| (l.clone(), l.clone())).collect();
        let picker = Picker {
            id_salt: "issue-labels",
            hint: words::label_hint(p),
            options: &labels,
            max: usize::MAX,
            free: opts.labels_free,
            add_word: words::add_word(p),
        };
        multi(ui, &picker, &mut form.label_query, &mut form.labels);
    }
    caption(ui, words::people_label(p));
    let people: Vec<(String, String)> = opts.people.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    let max = if form.loaded().is_some() { opts.max_assignees } else { usize::MAX };
    let picker =
        Picker { id_salt: "issue-people", hint: words::people_hint(p), options: &people, max, free: false, add_word: words::add_word(p) };
    multi(ui, &picker, &mut form.people_query, &mut form.people);
    if form.epic_shown() {
        caption(ui, words::epic_label(p));
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
    controls::checkbox(ui, &mut form.then_branch, "Create a branch for it afterwards");
    match &form.options {
        OptionsState::Loading => {
            ui.label(RichText::new(words::loading_text(p)).size(11.5).color(color::weak()));
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
        ui.add(egui::Label::new(RichText::new(format!("Couldn't create the {noun}: {e}")).color(color::red())).wrap());
    }
    submit
}

fn caption(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).size(12.0).color(color::weak()));
}

/// A picker over `items` that sets `chosen` to an item's key.
fn combo(ui: &mut Ui, id: &str, shown: String, width: f32, items: &[Named], chosen: &mut Option<String>) {
    egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).show_ui(ui, |ui| {
        for t in items {
            ui.selectable_value(chosen, Some(t.key.clone()), t.title.as_str());
        }
    });
}

/// Trello: creating cards needs a token that can write, with the link that allows one.
fn write_note(ui: &mut Ui, form: &IssueForm, msgs: &mut Vec<Msg>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Creating cards needs a Trello token allowed to read and write.").size(11.5).color(color::weak()));
        if let Some(url) = form.loaded().and_then(|o| o.write_token_url.clone())
            && ui.link(RichText::new("Allow a write token").size(11.5)).clicked()
        {
            msgs.push(Msg::OpenUrl(url));
        }
    });
}

/// Trello's board: the tracker's own, or one of the member's open boards to choose.
fn board(ui: &mut Ui, form: &IssueForm, msgs: &mut Vec<Msg>) {
    let p = form.provider();
    if form.board_fixed() {
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{}: {}", words::project_label(p), printable(&form.target.display()))).color(color::weak()));
        return;
    }
    caption(ui, words::project_label(p));
    let chosen = form.boards.iter().find(|b| b.key == form.project);
    let shown = chosen.map_or_else(|| "Choose a board".to_owned(), |b| printable(&b.title));
    egui::ComboBox::from_id_salt("issue-board").selected_text(shown).width(300.0).show_ui(ui, |ui| {
        for b in &form.boards {
            if ui.selectable_label(b.key == form.project, printable(&b.title)).clicked() && b.key != form.project {
                msgs.push(Msg::Issue(IssueMsg::SetProject(b.key.clone())));
            }
        }
    });
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
            let r = controls::field(
                ui,
                egui::TextEdit::singleline(&mut form.project_text).hint_text("Project key, e.g. PROJ").desired_width(200.0),
            );
            if r.lost_focus() && form.project_text.trim() != form.project {
                msgs.push(Msg::Issue(IssueMsg::SetProject(form.project_text.trim().to_owned())));
            }
        }
    }
}
