//! The Create issue dialog (`Modal::CreateIssue`; update/issue_create.rs,
//! ui/issue/create_dialog.rs): the tracker, the fields, the choices loaded from the service
//! (`Effect::Issue`), the live checks, and why the dialog can't be opened at all. Its words
//! follow the tracker (`issue_words`). Pure.

use super::AppState;
use super::issue_words;
use glowfly_core::forge::filters::Named;
use glowfly_core::forge::{self, CreateOptions, IssueField, NewIssue, Provider, Target, issues_in_app, type_required};

/// The labels, people, types and epics the service offers.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum OptionsState {
    /// Not asked: Jira without a project.
    #[default]
    NotAsked,
    Loading,
    Loaded(Box<CreateOptions>),
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct IssueForm {
    pub target: Target,
    /// The Jira project key or Trello board id the choices and the issue are for; empty
    /// elsewhere (and on Trello until a board is chosen).
    pub project: String,
    /// The project field's text, committed to `project` on Enter or when it loses focus.
    pub project_text: String,
    pub title: String,
    pub description: String,
    pub labels: Vec<String>,
    /// `Person::id`s.
    pub people: Vec<String>,
    /// A type's key (Trello: the list's id).
    pub kind: Option<String>,
    /// A priority's key (Bitbucket).
    pub priority: Option<String>,
    /// An epic's key (Azure Boards: the parent work item's id).
    pub epic: Option<String>,
    /// Open the New branch dialog for the issue once it is created.
    pub then_branch: bool,
    pub options: OptionsState,
    /// The project the options are for.
    pub options_for: String,
    /// Trello's open boards, to choose one from; kept when another board is chosen.
    pub boards: Vec<Named>,
    pub label_query: String,
    pub people_query: String,
    /// The issue is being created.
    pub busy: bool,
    /// The last create's failure.
    pub error: Option<String>,
    /// The fields edited and whether Create was tried, so a problem waits for them.
    pub touched: super::touched::Touched,
}

impl IssueForm {
    /// An empty form for `target`; the project starts as the tracker's Jira project filter or
    /// Trello board.
    pub fn new(target: Target) -> IssueForm {
        let project = match target.provider {
            Provider::Trello => target.owner.trim().to_owned(),
            p if p.is_jira() => target.project.trim().to_owned(),
            _ => String::new(),
        };
        IssueForm {
            project_text: project.clone(),
            project,
            target,
            title: String::new(),
            description: String::new(),
            labels: Vec::new(),
            people: Vec::new(),
            kind: None,
            priority: None,
            epic: None,
            then_branch: false,
            options: OptionsState::NotAsked,
            options_for: String::new(),
            boards: Vec::new(),
            label_query: String::new(),
            people_query: String::new(),
            busy: false,
            error: None,
            touched: Default::default(),
        }
    }

    pub fn provider(&self) -> Provider {
        self.target.provider
    }

    fn jira(&self) -> bool {
        self.provider().is_jira()
    }

    /// Trello with the tracker set to one board: the board isn't chosen in the dialog.
    pub fn board_fixed(&self) -> bool {
        self.provider() == Provider::Trello && !self.target.owner.is_empty()
    }

    /// "Summary" on Jira, "Name" on Trello, "Title" elsewhere.
    pub fn title_label(&self) -> &'static str {
        issue_words::title_label(self.provider())
    }

    pub fn heading(&self) -> String {
        issue_words::heading(self.provider())
    }

    pub fn loaded(&self) -> Option<&CreateOptions> {
        match &self.options {
            OptionsState::Loaded(o) => Some(&**o),
            _ => None,
        }
    }

    /// The chosen type's name.
    pub fn kind_title(&self) -> Option<&str> {
        let k = self.kind.as_deref()?;
        self.loaded().and_then(|o| o.types.iter().find(|t| t.key == k)).map(|t| t.title.as_str())
    }

    /// The Epic (Azure Boards: Parent) picker shows once epics are loaded, unless the issue is
    /// an epic itself.
    pub fn epic_shown(&self) -> bool {
        (self.jira() || self.provider() == Provider::AzureDevOps)
            && self.loaded().is_some_and(|o| !o.epics.is_empty())
            && !self.kind_title().is_some_and(|t| t.eq_ignore_ascii_case("epic"))
    }

    /// The issue as it will be sent: trimmed, the title on one line, labels once each.
    pub fn to_new(&self) -> NewIssue {
        let title = self.title.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        let mut labels: Vec<String> = Vec::new();
        for l in self.labels.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
            if !labels.iter().any(|x| x.to_lowercase() == l.to_lowercase()) {
                labels.push(l.to_owned());
            }
        }
        NewIssue {
            project: self.project.trim().to_owned(),
            title,
            description: self.description.trim().to_owned(),
            labels,
            assignees: self.people.clone(),
            kind: self.kind.clone(),
            priority: self.priority.clone(),
            epic: self.epic.clone().filter(|_| self.epic_shown()),
        }
    }

    /// What keeps the issue from being created, listed live in the dialog. The type is asked
    /// for only once the types are loaded (or failed to).
    pub fn problems(&self) -> Vec<String> {
        self.field_problems().into_iter().map(|(_, why)| why).collect()
    }

    /// `problems`, each with the field it is about.
    pub fn field_problems(&self) -> Vec<(IssueField, String)> {
        let mut out = self.to_new().field_problems(self.provider());
        if matches!(self.options, OptionsState::NotAsked | OptionsState::Loading) {
            out.retain(|(f, _)| *f != IssueField::Type);
        }
        if let Some(o) = self.loaded()
            && self.people.len() > o.max_assignees
        {
            out.push((IssueField::Assignees, too_many(o.max_assignees)));
        }
        out
    }

    /// The problem with `field`, if it has one.
    pub fn problem(&self, field: IssueField) -> Option<String> {
        self.field_problems().into_iter().find(|(f, _)| *f == field).map(|(_, why)| why)
    }

    /// Why Create is disabled: the first problem, or the types still loading on a tracker that
    /// needs one.
    pub fn submit_blocked(&self) -> Option<String> {
        if let Some(p) = self.problems().into_iter().next() {
            return Some(p);
        }
        (type_required(self.provider()) && self.kind.is_none()).then(|| issue_words::waiting_text(self.provider()).to_owned())
    }

    /// The choices for `project` arrived: the default type and priority are picked, and chosen
    /// people, epics and (where only known labels are allowed) labels the service doesn't offer
    /// are dropped. Trello's boards are kept for the Board picker.
    pub fn options_loaded(&mut self, opts: CreateOptions) {
        if !opts.boards.is_empty() {
            self.boards = opts.boards.clone();
        }
        let offered = |k: &str| opts.types.iter().any(|t| t.key == k);
        if !self.kind.as_deref().is_some_and(offered) {
            self.kind = match opts.default_type.as_deref().filter(|d| offered(d)) {
                Some(d) => Some(d.to_owned()),
                None if self.jira() => {
                    opts.types.iter().find(|t| t.title.eq_ignore_ascii_case("task")).or(opts.types.first()).map(|t| t.key.clone())
                }
                None => opts.types.iter().find(|t| t.key == "issue").or(opts.types.first()).map(|t| t.key.clone()),
            };
        }
        let priority_offered = |k: &str| opts.priorities.iter().any(|o| o.key == k);
        if !self.priority.as_deref().is_some_and(priority_offered) {
            self.priority = opts.default_priority.clone().filter(|d| priority_offered(d));
        }
        self.people.retain(|p| opts.people.iter().any(|o| &o.id == p));
        if self.epic.as_ref().is_some_and(|e| !opts.epics.iter().any(|o| &o.key == e)) {
            self.epic = None;
        }
        if !opts.labels_free {
            self.labels.retain(|l| opts.labels.iter().any(|o| o.eq_ignore_ascii_case(l)));
        }
        self.options = OptionsState::Loaded(Box::new(opts));
    }

    /// Another project (Trello: board): its types, epics and people (Trello: and labels)
    /// differ, so those choices are cleared.
    pub fn set_project(&mut self, p: &str) {
        self.project = p.trim().to_owned();
        self.project_text = self.project.clone();
        self.kind = None;
        self.epic = None;
        self.people.clear();
        if self.provider() == Provider::Trello {
            self.labels.clear();
        }
        self.options = OptionsState::NotAsked;
    }
}

/// The note on assignees past the service's maximum.
pub fn too_many(max: usize) -> String {
    if max == 1 { "Only one assignee on this service".to_owned() } else { format!("At most {max} assignees") }
}

/// Why an issue can't be created for the active repository now; None when it can.
pub fn create_blocked(st: &AppState) -> Option<String> {
    let none = || Some("No issue tracker for this repository".to_owned());
    let Some(fs) = st.forge.as_ref() else { return none() };
    let Some(t) = fs.tracker.as_ref() else {
        if fs.host.as_ref().is_some_and(|h| h.provider == Provider::BitbucketDataCenter) {
            return Some(
                "Bitbucket Data Center has no issue tracker of its own; choose Jira under Repo-specific preferences > Issue tracker".into(),
            );
        }
        return none();
    };
    if !issues_in_app(t.provider) {
        return Some(format!("Creating {} {}s in Glowfly isn't supported yet", fs.tracker_name(), fs.issue_noun()));
    }
    if !st.runs_git_unasked() {
        return Some("Trust this repository to create issues".into());
    }
    let account = t.account();
    if !forge::may_contact(&account, &st.settings.connections) {
        return Some(format!("Connect {} at {} in Preferences first", account.provider.name(), account.display()));
    }
    None
}

#[cfg(test)]
#[path = "issue_form_tests.rs"]
mod tests;
