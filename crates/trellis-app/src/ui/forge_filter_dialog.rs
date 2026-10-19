//! The filter dialog (`Modal::ForgeFilter`): a saved pull request or issue filter's name and
//! conditions. Controls for conditions the service can't apply are disabled with the reason;
//! labels, milestones and sprints come from the service once loaded.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::{FilterMsg, Msg};
use crate::state::ForgeState;
use crate::state::forge_filter_form::FilterForm;
use crate::state::forge_filters::{OptionsState, suggestions};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, Key, RichText};
use trellis_core::forge::filters::matching::{self, Field};
use trellis_core::forge::filters::{Draft, Milestone, Review, Sprint, Who};
use trellis_core::forge::{FilterKind, FilterOptions};
use trellis_core::settings::RepoForgeFilters;

/// Suggested labels shown under the labels field.
const MAX_CHIPS: usize = 12;
const WIDTH: f32 = 230.0;
/// The width of the labels left of the controls.
const LABEL_W: f32 = 100.0;

/// Draws the dialog; returns true when it should close. The name field takes the keyboard
/// focus when nothing has it, unless `cx.keyboard` is false (a credential prompt is above it).
pub fn show(
    ui: &mut egui::Ui,
    cx: Cx,
    form: &mut FilterForm,
    fs: Option<&ForgeState>,
    existing: &RepoForgeFilters,
    msgs: &mut Vec<Msg>,
) -> bool {
    let target = fs.and_then(|f| crate::state::forge_filters::target_for(f, form.kind));
    let mut d = Dialog::new(cx, form.title());
    if let Some(t) = &target {
        d = d.subtitle(format!("{} · {}", t.provider.name(), t.display()));
    }
    let mut d = d.begin(ui);
    let submit = d.body(ui, |ui| body(ui, form, fs, existing, cx.keyboard));
    let label = if form.editing.is_some() { "Save" } else { "Create filter" };
    let ok = form.problems(existing).is_empty();
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary(label).enabled(ok)).submit(submit));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::ForgeFilter(FilterMsg::Save));
    }
    out.dismiss
}

/// The name and the conditions; returns whether Enter left the name field.
fn body(ui: &mut egui::Ui, form: &mut FilterForm, fs: Option<&ForgeState>, existing: &RepoForgeFilters, focus: bool) -> bool {
    let r = controls::field(ui, egui::TextEdit::singleline(&mut form.name).hint_text("Filter name").desired_width(f32::INFINITY));
    form_message::track(&mut form.touched, "name", &r);
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus && ui.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
    form_message::field(ui, &form.touched, "name", form.name_problem(existing).map(Message::error));
    ui.add_space(8.0);
    let options = fs.map(|f| (suggestions(f, form.kind), f.filters.options(form.kind).clone()));
    let (loaded, state) = options.unwrap_or((Some(FilterOptions::default()), OptionsState::NotAsked));
    let loaded = loaded.unwrap_or_default();
    let loading = state == OptionsState::Loading;
    ui.spacing_mut().item_spacing.y = 6.0;
    let mut f = Fields { form, loaded: &loaded, loading };
    match f.form.kind {
        FilterKind::Prs => f.pr_rows(ui),
        FilterKind::Issues => f.issue_rows(ui),
    }
    if let OptionsState::Failed(e) = &state {
        ui.add_space(4.0);
        ui.label(RichText::new(format!("Couldn't load the choices: {e}")).size(type_scale::CAPTION).color(color::faint()));
    }
    // The conditions come from the filter on show, not from typing: they show at once.
    for p in form.condition_problems() {
        ui.add_space(6.0);
        form_message::form(ui, Message::error(p));
    }
    submit
}

struct Fields<'a> {
    form: &'a mut FilterForm,
    loaded: &'a FilterOptions,
    loading: bool,
}

impl Fields<'_> {
    /// Why the service can't apply `field`, when it can't.
    fn why(&self, field: Field) -> Option<String> {
        self.form.provider.and_then(|p| matching::unsupported_field(p, self.form.kind, field))
    }

    fn pr_rows(&mut self, ui: &mut egui::Ui) {
        let why = self.why(Field::Author);
        row(ui, "Author", why, |ui| combo(ui, "author", &mut self.form.c.author, &[(Who::Anyone, "Anyone"), (Who::Me, "Me")]));
        self.assignee(ui);
        let why = self.why(Field::Review);
        let reviews = [(Review::Any, "Any"), (Review::WaitingForMe, "Waiting for my review")];
        row(ui, "Review", why, |ui| combo(ui, "review", &mut self.form.c.review, &reviews));
        let why = self.why(Field::Draft);
        let drafts = [(Draft::Any, "Any"), (Draft::Ready, "Ready for review only"), (Draft::Drafts, "Drafts only")];
        row(ui, "Draft", why, |ui| combo(ui, "draft", &mut self.form.c.draft, &drafts));
        let why = self.why(Field::Base);
        row(ui, "Target branch", why, |ui| {
            controls::field(ui, egui::TextEdit::singleline(&mut self.form.c.base).hint_text("Any branch").desired_width(WIDTH))
        });
        self.labels(ui);
        self.milestone(ui);
    }

    fn issue_rows(&mut self, ui: &mut egui::Ui) {
        self.assignee(ui);
        let why = self.why(Field::Author);
        row(ui, "Created by", why, |ui| combo(ui, "creator", &mut self.form.c.author, &[(Who::Anyone, "Anyone"), (Who::Me, "Me")]));
        let why = self.why(Field::Mentions);
        row(ui, "Mentions", why, |ui| controls::checkbox(ui, &mut self.form.c.mentions_me, "Mentions me"));
        self.labels(ui);
        self.milestone(ui);
        if self.form.provider.is_some_and(|p| p.is_jira()) {
            self.sprint(ui);
        }
    }

    fn assignee(&mut self, ui: &mut egui::Ui) {
        let why = self.why(Field::Assignee);
        let who = [(Who::Anyone, "Anyone"), (Who::Me, "Me"), (Who::Nobody, "Nobody")];
        row(ui, "Assignee", why, |ui| combo(ui, "assignee", &mut self.form.c.assignee, &who));
    }

    fn labels(&mut self, ui: &mut egui::Ui) {
        let noun = self.form.provider.map(matching::labels_noun).unwrap_or("Labels");
        let why = self.why(Field::Labels);
        row(ui, noun, why.clone(), |ui| {
            controls::field(ui, egui::TextEdit::singleline(&mut self.form.labels).hint_text("Comma-separated").desired_width(WIDTH))
        });
        let typed = self.form.label_list();
        let offered: Vec<String> =
            self.loaded.labels.iter().filter(|l| !typed.iter().any(|t| t.eq_ignore_ascii_case(l))).take(MAX_CHIPS).cloned().collect();
        if offered.is_empty() || why.is_some() {
            return;
        }
        let mut rect = ui.available_rect_before_wrap();
        rect.min.x += LABEL_W + ui.spacing().item_spacing.x;
        rect.max.x = rect.min.x + WIDTH;
        let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(layout), |ui| {
            for l in offered {
                if ui.small_button(l.as_str()).on_hover_text(format!("Add {l}")).clicked() {
                    self.form.add_label(&l);
                }
            }
        });
    }

    fn milestone(&mut self, ui: &mut egui::Ui) {
        let noun = self.form.provider.map(matching::milestone_noun).unwrap_or("Milestone");
        let why = self.why(Field::Milestone);
        let mut choices: Vec<(Milestone, String)> = vec![(Milestone::Any, "Any".into()), (Milestone::None, "None".into())];
        choices.extend(
            self.loaded.milestones.iter().map(|m| (Milestone::Named { key: m.key.clone(), title: m.title.clone() }, m.title.clone())),
        );
        // A saved one no longer listed stays (GitHub ones are only kept while listed).
        if let Milestone::Named { title, .. } = &self.form.c.milestone
            && !choices.iter().any(|(m, _)| *m == self.form.c.milestone)
        {
            choices.push((self.form.c.milestone.clone(), title.clone()));
        }
        let loading = self.loading;
        row(ui, noun, why, |ui| combo_loading(ui, "milestone", &mut self.form.c.milestone, &choices, loading));
    }

    fn sprint(&mut self, ui: &mut egui::Ui) {
        let why = self.why(Field::Sprint);
        let mut choices: Vec<(Sprint, String)> = vec![(Sprint::Any, "Any".into()), (Sprint::Open, "Open sprints".into())];
        for s in &self.loaded.sprints {
            if let Ok(id) = s.key.parse() {
                choices.push((Sprint::Named { id, name: s.title.clone() }, s.title.clone()));
            }
        }
        if let Sprint::Named { name, .. } = &self.form.c.sprint
            && !choices.iter().any(|(s, _)| *s == self.form.c.sprint)
        {
            choices.push((self.form.c.sprint.clone(), name.clone()));
        }
        let loading = self.loading;
        row(ui, "Sprint", why, |ui| combo_loading(ui, "sprint", &mut self.form.c.sprint, &choices, loading));
    }
}

/// One labelled row of the grid; a control the service can't use is disabled with `why` on hover.
fn row(ui: &mut egui::Ui, label: &str, why: Option<String>, add: impl FnOnce(&mut egui::Ui) -> egui::Response) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(LABEL_W, ui.spacing().interact_size.y), egui::Sense::hover());
        let font = egui::TextStyle::Body.resolve(ui.style());
        let col = if why.is_some() { color::faint() } else { color::text() };
        ui.painter().text(rect.left_center(), egui::Align2::LEFT_CENTER, label, font, col);
        ui.add_enabled_ui(why.is_none(), |ui| {
            let r = add(ui);
            if let Some(why) = &why {
                r.on_disabled_hover_text(why);
            }
        });
    });
}

fn combo<T: PartialEq + Clone>(ui: &mut egui::Ui, id: &str, value: &mut T, choices: &[(T, &str)]) -> egui::Response {
    let owned: Vec<(T, String)> = choices.iter().map(|(v, l)| (v.clone(), (*l).to_owned())).collect();
    combo_loading(ui, id, value, &owned, false)
}

/// A combo box of `choices`; while `loading` it says so in its list and beside it.
fn combo_loading<T: PartialEq + Clone>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    choices: &[(T, String)],
    loading: bool,
) -> egui::Response {
    let selected = choices.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let r = egui::ComboBox::from_id_salt(("forge-filter", id)).selected_text(selected).width(WIDTH).show_ui(ui, |ui| {
        for (v, l) in choices {
            ui.selectable_value(value, v.clone(), l.as_str());
        }
        if loading {
            ui.label(RichText::new("Loading…").color(color::faint()));
        }
    });
    if loading {
        ui.label(RichText::new("Loading…").size(type_scale::CAPTION).color(color::faint()));
    }
    r.response
}
