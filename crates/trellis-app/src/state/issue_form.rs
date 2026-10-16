//! The Create issue dialog (`Modal::CreateIssue`; update/issue_create.rs,
//! ui/issue/create_dialog.rs): the tracker, the fields, the choices loaded from the service
//! (`Effect::Issue`), the live checks, and why the dialog can't be opened at all. Pure.

use super::AppState;
use trellis_core::forge::{self, CreateOptions, NewIssue, Provider, Target, issues_in_app};

/// The labels, people, types and epics the service offers.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum OptionsState {
    /// Not asked: Jira without a project.
    #[default]
    NotAsked,
    Loading,
    Loaded(CreateOptions),
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct IssueForm {
    pub target: Target,
    /// The Jira project key the choices and the issue are for; empty elsewhere.
    pub project: String,
    /// The project field's text, committed to `project` on Enter or when it loses focus.
    pub project_text: String,
    pub title: String,
    pub description: String,
    pub labels: Vec<String>,
    /// `Person::id`s.
    pub people: Vec<String>,
    /// A type's key.
    pub kind: Option<String>,
    /// An epic's key.
    pub epic: Option<String>,
    /// Open the New branch dialog for the issue once it is created.
    pub then_branch: bool,
    pub options: OptionsState,
    /// The project the options are for.
    pub options_for: String,
    pub label_query: String,
    pub people_query: String,
    /// The issue is being created.
    pub busy: bool,
    /// The last create's failure.
    pub error: Option<String>,
}

impl IssueForm {
    /// An empty form for `target`; on Jira the project starts as the tracker's project filter.
    pub fn new(target: Target) -> IssueForm {
        let project = if target.provider.is_jira() { target.project.trim().to_owned() } else { String::new() };
        IssueForm {
            project_text: project.clone(),
            project,
            target,
            title: String::new(),
            description: String::new(),
            labels: Vec::new(),
            people: Vec::new(),
            kind: None,
            epic: None,
            then_branch: false,
            options: OptionsState::NotAsked,
            options_for: String::new(),
            label_query: String::new(),
            people_query: String::new(),
            busy: false,
            error: None,
        }
    }

    pub fn provider(&self) -> Provider {
        self.target.provider
    }

    fn jira(&self) -> bool {
        self.provider().is_jira()
    }

    /// "Summary" on Jira, "Title" elsewhere.
    pub fn title_label(&self) -> &'static str {
        if self.jira() { "Summary" } else { "Title" }
    }

    pub fn heading(&self) -> &'static str {
        "Create issue"
    }

    pub fn loaded(&self) -> Option<&CreateOptions> {
        match &self.options {
            OptionsState::Loaded(o) => Some(o),
            _ => None,
        }
    }

    /// The chosen type's name.
    pub fn kind_title(&self) -> Option<&str> {
        let k = self.kind.as_deref()?;
        self.loaded().and_then(|o| o.types.iter().find(|t| t.key == k)).map(|t| t.title.as_str())
    }

    /// The Epic picker shows on Jira once epics are loaded, unless the issue is an epic itself.
    pub fn epic_shown(&self) -> bool {
        self.jira()
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
            epic: self.epic.clone().filter(|_| self.epic_shown()),
        }
    }

    /// What keeps the issue from being created, listed live in the dialog. The issue type is
    /// asked for only once the types are loaded (or failed to).
    pub fn problems(&self) -> Vec<String> {
        let mut out = self.to_new().problems(self.provider());
        if matches!(self.options, OptionsState::NotAsked | OptionsState::Loading) {
            out.retain(|p| p != "Choose an issue type.");
        }
        if let Some(o) = self.loaded()
            && self.people.len() > o.max_assignees
        {
            out.push(too_many(o.max_assignees));
        }
        out
    }

    /// Why Create is disabled: the first problem, or (Jira) types still loading.
    pub fn submit_blocked(&self) -> Option<String> {
        if let Some(p) = self.problems().into_iter().next() {
            return Some(p);
        }
        (self.jira() && self.kind.is_none()).then(|| "Wait for the issue types to load.".to_owned())
    }

    /// The choices for `project` arrived: the default type is picked, and chosen people, epics
    /// and (where only known labels are allowed) labels the service doesn't offer are dropped.
    pub fn options_loaded(&mut self, opts: CreateOptions) {
        if !self.kind.as_ref().is_some_and(|k| opts.types.iter().any(|t| &t.key == k)) {
            self.kind = if self.jira() {
                opts.types.iter().find(|t| t.title.eq_ignore_ascii_case("task")).or(opts.types.first()).map(|t| t.key.clone())
            } else {
                opts.types.iter().find(|t| t.key == "issue").or(opts.types.first()).map(|t| t.key.clone())
            };
        }
        self.people.retain(|p| opts.people.iter().any(|o| &o.id == p));
        if self.epic.as_ref().is_some_and(|e| !opts.epics.iter().any(|o| &o.key == e)) {
            self.epic = None;
        }
        if !opts.labels_free {
            self.labels.retain(|l| opts.labels.iter().any(|o| o.eq_ignore_ascii_case(l)));
        }
        self.options = OptionsState::Loaded(opts);
    }

    /// Another project: its types, epics and people differ, so those choices are cleared.
    pub fn set_project(&mut self, p: &str) {
        self.project = p.trim().to_owned();
        self.project_text = self.project.clone();
        self.kind = None;
        self.epic = None;
        self.people.clear();
        self.options = OptionsState::NotAsked;
    }
}

/// The note on assignees past the service's maximum.
pub fn too_many(max: usize) -> String {
    if max == 1 { "Only one assignee on this service".to_owned() } else { format!("At most {max} assignees") }
}

/// Why an issue can't be created for the active repository now; None when it can.
pub fn create_blocked(st: &AppState) -> Option<String> {
    let Some(fs) = st.forge.as_ref() else { return Some("No issue tracker for this repository".into()) };
    let Some(t) = fs.tracker.as_ref() else { return Some("No issue tracker for this repository".into()) };
    if !issues_in_app(t.provider) {
        return Some(format!("Creating {} {}s in Trellis isn't supported yet", fs.tracker_name(), fs.issue_noun()));
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
mod tests {
    use super::*;
    use crate::state::ForgeState;
    use std::time::Instant;
    use trellis_core::forge::filters::Named;
    use trellis_core::forge::{Account, Person};
    use trellis_core::settings::Settings;

    fn github() -> Target {
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
    }

    fn jira(project: &str) -> Target {
        Target::jira(&Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() }, project)
    }

    fn named(key: &str, title: &str) -> Named {
        Named { key: key.into(), title: title.into() }
    }

    fn person(id: &str) -> Person {
        Person { id: id.into(), name: id.to_uppercase() }
    }

    #[test]
    fn labels_and_the_issue_sent() {
        let mut f = IssueForm::new(github());
        assert_eq!((f.title_label(), f.heading()), ("Title", "Create issue"));
        assert_eq!(IssueForm::new(jira("P")).title_label(), "Summary");
        assert_eq!(IssueForm::new(jira(" P ")).project, "P", "the project filter");
        f.title = "  Crash\r\n on   start \n".into();
        f.description = "\n Steps \n\n".into();
        f.labels = vec!["Bug".into(), "ui".into(), "bug".into(), " ".into()];
        let n = f.to_new();
        assert_eq!((n.title.as_str(), n.description.as_str()), ("Crash on   start", "Steps"));
        assert_eq!(n.labels, ["Bug", "ui"]);
    }

    #[test]
    fn problems_include_the_assignee_maximum() {
        let mut f = IssueForm::new(github());
        assert_eq!(f.problems(), ["Enter a title."]);
        assert_eq!(f.submit_blocked().as_deref(), Some("Enter a title."));
        f.title = "Crash".into();
        f.people = vec!["a".into(), "b".into()];
        assert!(f.problems().is_empty(), "nothing known about the maximum yet");
        f.options_loaded(CreateOptions { people: vec![person("a"), person("b")], max_assignees: 1, ..Default::default() });
        assert_eq!(f.problems(), ["Only one assignee on this service"]);
        assert_eq!(too_many(10), "At most 10 assignees");
        f.people.pop();
        assert!(f.submit_blocked().is_none());
    }

    #[test]
    fn loaded_options_pick_a_type_and_drop_what_is_not_offered() {
        let mut f = IssueForm::new(jira("P"));
        f.title = "Login".into();
        assert!(f.problems().is_empty(), "the type is asked for once the types are loaded");
        assert_eq!(f.submit_blocked().as_deref(), Some("Wait for the issue types to load."));
        f.people = vec!["gone".into(), "acc-1".into()];
        f.epic = Some("P-9".into());
        let opts = CreateOptions {
            types: vec![named("1", "Bug"), named("2", "Task"), named("3", "Epic")],
            people: vec![person("acc-1")],
            epics: vec![named("P-1", "Accounts")],
            labels_free: true,
            max_assignees: 1,
            ..Default::default()
        };
        f.options_loaded(opts.clone());
        assert_eq!(f.kind.as_deref(), Some("2"), "Task");
        assert_eq!((f.people.as_slice(), f.epic.as_deref()), (&["acc-1".to_owned()][..], None));
        assert!(f.epic_shown());
        f.epic = Some("P-1".into());
        assert_eq!(f.to_new().epic.as_deref(), Some("P-1"));
        f.kind = Some("3".into());
        assert!(!f.epic_shown(), "an epic has no epic");
        assert_eq!(f.to_new().epic, None);
        let mut f = IssueForm::new(jira("P"));
        f.options_loaded(CreateOptions { types: vec![named("7", "Story")], ..opts });
        assert_eq!(f.kind.as_deref(), Some("7"), "else the first");

        let mut gl = IssueForm::new(Target { provider: Provider::GitLab, host: "gitlab.com".into(), ..github() });
        gl.labels = vec!["new-one".into()];
        gl.options_loaded(CreateOptions {
            types: vec![named("incident", "Incident"), named("issue", "Issue")],
            labels_free: true,
            ..Default::default()
        });
        assert_eq!((gl.kind.as_deref(), gl.labels.len()), (Some("issue"), 1));
        let mut gh = IssueForm::new(github());
        gh.labels = vec!["BUG".into(), "made-up".into()];
        gh.options_loaded(CreateOptions { labels: vec!["bug".into()], ..Default::default() });
        assert_eq!((gh.kind, gh.labels), (None, vec!["BUG".to_owned()]), "only known labels on GitHub");
    }

    #[test]
    fn another_project_clears_its_choices() {
        let mut f = IssueForm::new(jira("P"));
        f.kind = Some("2".into());
        f.epic = Some("P-1".into());
        f.people = vec!["acc-1".into()];
        f.labels = vec!["ui".into()];
        f.set_project(" Q ");
        assert_eq!((f.project.as_str(), f.project_text.as_str()), ("Q", "Q"));
        assert_eq!((f.kind, f.epic, f.people.len(), f.labels.len()), (None, None, 0, 1));
        assert_eq!(f.options, OptionsState::NotAsked);
    }

    #[test]
    fn why_creating_is_blocked() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        assert_eq!(create_blocked(&st).as_deref(), Some("No issue tracker for this repository"));
        st.forge = Some(ForgeState::new(Some(github()), "origin".into(), None));
        assert_eq!(create_blocked(&st).as_deref(), Some("No issue tracker for this repository"));
        let bb = Target { provider: Provider::Bitbucket, host: "bitbucket.org".into(), ..github() };
        st.forge = Some(ForgeState::new(Some(bb.clone()), "origin".into(), Some(bb)));
        assert_eq!(create_blocked(&st).as_deref(), Some("Creating Bitbucket issues in Trellis isn't supported yet"));
        let az = Target { provider: Provider::AzureDevOps, host: "dev.azure.com".into(), ..github() };
        st.forge = Some(ForgeState::new(None, String::new(), Some(az)));
        assert_eq!(create_blocked(&st).as_deref(), Some("Creating Azure Boards work items in Trellis isn't supported yet"));
        let ghe = Target { provider: Provider::GitHubEnterprise, host: "git.acme.io".into(), ..github() };
        st.forge = Some(ForgeState::new(Some(ghe.clone()), "origin".into(), Some(ghe)));
        assert_eq!(create_blocked(&st).as_deref(), Some("Connect GitHub Enterprise Server at git.acme.io in Preferences first"));
        st.forge = Some(ForgeState::new(Some(github()), "origin".into(), Some(github())));
        assert_eq!(create_blocked(&st), None);
    }
}
