//! The words the issue panel and the Create dialog use, per tracker: what an issue is called
//! ("work item", "card"), the dialog's heading and captions, the Details rows' labels and the
//! loading texts. Pure.

use trellis_core::forge::{self, Provider};

/// "issue", "work item" (Azure Boards) or "card" (Trello).
pub fn noun(p: Provider) -> &'static str {
    forge::noun(p)
}

/// The dialog's heading and its submit button: "Create issue", "Create work item", "Create card".
pub fn heading(p: Provider) -> String {
    format!("Create {}", noun(p))
}

pub fn title_label(p: Provider) -> &'static str {
    match p {
        Provider::Trello => "Name",
        _ if p.is_jira() => "Summary",
        _ => "Title",
    }
}

/// The type picker: a Bitbucket kind, a Trello list, else a type.
pub fn type_label(p: Provider) -> &'static str {
    match p {
        Provider::Bitbucket => "Kind",
        Provider::Trello => "List",
        _ => "Type",
    }
}

/// Where the issue goes: a Jira project, a Trello board.
pub fn project_label(p: Provider) -> &'static str {
    if p == Provider::Trello { "Board" } else { "Project" }
}

pub fn labels_label(p: Provider) -> &'static str {
    if p == Provider::AzureDevOps { "Tags" } else { "Labels" }
}

pub fn people_label(p: Provider) -> &'static str {
    match p {
        Provider::Trello => "Members",
        Provider::AzureDevOps => "Assigned to",
        _ => "Assignees",
    }
}

/// The parent picker and row: a Jira epic, an Azure parent work item.
pub fn epic_label(p: Provider) -> &'static str {
    if p == Provider::AzureDevOps { "Parent" } else { "Epic" }
}

/// The state row: a Jira status, a Trello list, else a state.
pub fn state_label(p: Provider) -> &'static str {
    match p {
        Provider::Trello => "List",
        _ if p.is_jira() => "Status",
        _ => "State",
    }
}

/// The row for `Issue::kind`: Trello keeps the card's board there.
pub fn kind_row_label(p: Provider) -> &'static str {
    match p {
        Provider::Trello => "Board",
        Provider::Bitbucket => "Kind",
        _ => "Type",
    }
}

/// "Fix version" on Jira, "Milestone" on code hosts.
pub fn milestone_label(p: Provider) -> &'static str {
    if p.is_jira() { "Fix version" } else { "Milestone" }
}

pub fn label_hint(p: Provider) -> &'static str {
    if p == Provider::AzureDevOps { "Search tags" } else { "Search labels" }
}

pub fn people_hint(p: Provider) -> &'static str {
    if p == Provider::Trello { "Search members" } else { "Search people" }
}

/// What a typed label is added as: "tag" on Azure Boards, else "label".
pub fn add_word(p: Provider) -> &'static str {
    if p == Provider::AzureDevOps { "tag" } else { "label" }
}

/// While the dialog's choices load.
pub fn loading_text(p: Provider) -> &'static str {
    match p {
        Provider::Trello => "Loading lists, labels and members…",
        Provider::AzureDevOps => "Loading types, tags and people…",
        _ => "Loading labels, people and types…",
    }
}

/// Why Create waits on a tracker that needs a type, while the types load.
pub fn waiting_text(p: Provider) -> &'static str {
    match p {
        Provider::AzureDevOps => "Wait for the work item types to load.",
        Provider::Trello => "Wait for the lists to load.",
        _ => "Wait for the issue types to load.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_per_tracker() {
        use Provider::*;
        type Row = (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str);
        let row = |p: Provider| -> Row {
            (title_label(p), type_label(p), labels_label(p), people_label(p), epic_label(p), state_label(p), kind_row_label(p))
        };
        assert_eq!(row(GitHub), ("Title", "Type", "Labels", "Assignees", "Epic", "State", "Type"));
        assert_eq!(row(GitLab), ("Title", "Type", "Labels", "Assignees", "Epic", "State", "Type"));
        assert_eq!(row(Bitbucket), ("Title", "Kind", "Labels", "Assignees", "Epic", "State", "Kind"));
        assert_eq!(row(AzureDevOps), ("Title", "Type", "Tags", "Assigned to", "Parent", "State", "Type"));
        assert_eq!(row(JiraCloud), ("Summary", "Type", "Labels", "Assignees", "Epic", "Status", "Type"));
        assert_eq!(row(JiraDataCenter), ("Summary", "Type", "Labels", "Assignees", "Epic", "Status", "Type"));
        assert_eq!(row(Trello), ("Name", "List", "Labels", "Members", "Epic", "List", "Board"));
        let headings: Vec<String> = [GitHub, AzureDevOps, Trello, JiraCloud].map(heading).to_vec();
        assert_eq!(headings, ["Create issue", "Create work item", "Create card", "Create issue"]);
        assert_eq!((project_label(JiraCloud), project_label(Trello)), ("Project", "Board"));
        assert_eq!((milestone_label(GitHub), milestone_label(JiraDataCenter)), ("Milestone", "Fix version"));
        assert_eq!(
            (label_hint(AzureDevOps), label_hint(GitHub), people_hint(Trello), people_hint(GitLab)),
            ("Search tags", "Search labels", "Search members", "Search people")
        );
        assert_eq!((add_word(AzureDevOps), add_word(GitLab)), ("tag", "label"));
        assert_eq!(loading_text(Trello), "Loading lists, labels and members…");
        assert_eq!(loading_text(AzureDevOps), "Loading types, tags and people…");
        assert_eq!(loading_text(GitHub), "Loading labels, people and types…");
        assert_eq!(waiting_text(JiraCloud), "Wait for the issue types to load.");
        assert_eq!(waiting_text(AzureDevOps), "Wait for the work item types to load.");
        assert_eq!(waiting_text(Trello), "Wait for the lists to load.");
    }
}
