use super::matching::{matching_prs, needs_me, problems, unsupported};
use super::*;
use crate::forge::{Provider, PullRequest};

fn pr(id: u64, facets: PrFacets) -> PullRequest {
    PullRequest {
        id,
        title: format!("P{id}"),
        description: String::new(),
        source: format!("b{id}"),
        target: "main".into(),
        author: String::new(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        checks: None,
        url: String::new(),
        facets,
    }
}

fn me() -> Me {
    Me { id: "ada".into(), name: "Ada".into(), ..Default::default() }
}

fn ids(found: Option<Vec<&PullRequest>>) -> Vec<u64> {
    found.expect("me is known").iter().map(|p| p.id).collect()
}

#[test]
fn builtins_hold_their_conditions() {
    let c = |b: Builtin| b.conditions();
    assert_eq!(c(Builtin::AllPrs), Conditions::default());
    assert_eq!(c(Builtin::MyPrs), Conditions { author: Who::Me, ..Default::default() });
    assert_eq!(c(Builtin::NeedsMyReview), Conditions { review: Review::WaitingForMe, ..Default::default() });
    assert_eq!(c(Builtin::PrsAssignedToMe), Conditions { assignee: Who::Me, ..Default::default() });
    assert_eq!(c(Builtin::IssuesAssignedToMe), Conditions { assignee: Who::Me, ..Default::default() });
    assert_eq!(c(Builtin::IssuesCreatedByMe), Conditions { author: Who::Me, ..Default::default() });
    assert_eq!(c(Builtin::IssuesMentioningMe), Conditions { mentions_me: true, ..Default::default() });
    assert!(Builtin::PRS.iter().all(|b| b.kind() == FilterKind::Prs));
    assert!(Builtin::ISSUES.iter().all(|b| b.kind() == FilterKind::Issues));
    assert_eq!(FilterChoice::default_for(FilterKind::Issues), FilterChoice::Builtin(Builtin::IssuesAssignedToMe));
    assert_eq!(Builtin::NeedsMyReview.label(), "Needs my review");
}

#[test]
fn choices_and_conditions_survive_a_round_trip() {
    let c = Conditions {
        author: Who::Me,
        assignee: Who::Nobody,
        labels: vec!["bug".into(), "good first".into()],
        milestone: Milestone::Named { key: "3".into(), title: "v1".into() },
        sprint: Sprint::Named { id: 42, name: "Sprint 9".into() },
        draft: Draft::Ready,
        base: "main".into(),
        ..Default::default()
    };
    let back: Conditions = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(back, c);
    for choice in [FilterChoice::Builtin(Builtin::NeedsMyReview), FilterChoice::Saved(7)] {
        let back: FilterChoice = serde_json::from_str(&serde_json::to_string(&choice).unwrap()).unwrap();
        assert_eq!(back, choice);
    }
    assert_eq!(serde_json::from_str::<Conditions>("{}").unwrap(), Conditions::default());
    assert_eq!(serde_json::from_str::<SavedFilter>("{}").unwrap(), SavedFilter::default());
}

#[test]
fn matches_loaded_pull_requests() {
    let prs = vec![
        pr(1, PrFacets { author_id: "Ada".into(), labels: vec!["Bug".into(), "ui".into()], ..Default::default() }),
        pr(
            2,
            PrFacets {
                author_id: "grace".into(),
                reviewer_ids: vec!["ada".into(), "linus".into()],
                awaiting_ids: vec!["ada".into()],
                assignee_ids: vec!["ADA".into()],
                milestone: Some("v1".into()),
                ..Default::default()
            },
        ),
        pr(3, PrFacets { author_id: "linus".into(), reviewer_ids: vec!["ada".into()], ..Default::default() }),
    ];
    let m = me();
    let find = |c: Conditions| ids(matching_prs(&prs, &c, Some(&m)));
    assert_eq!(find(Builtin::AllPrs.conditions()), [1, 2, 3]);
    assert_eq!(find(Builtin::MyPrs.conditions()), [1], "ids ignore case");
    // Ada already voted on 3: only 2 still waits for her.
    assert_eq!(find(Builtin::NeedsMyReview.conditions()), [2]);
    assert_eq!(find(Builtin::PrsAssignedToMe.conditions()), [2]);
    assert_eq!(find(Conditions { assignee: Who::Nobody, ..Default::default() }), [1, 3]);
    assert_eq!(find(Conditions { labels: vec!["bug".into(), "UI".into()], ..Default::default() }), [1]);
    assert!(find(Conditions { labels: vec!["bug".into(), "docs".into()], ..Default::default() }).is_empty(), "all of them");
    assert_eq!(find(Conditions { milestone: Milestone::None, ..Default::default() }), [1, 3]);
    let v1 = Milestone::Named { key: "1".into(), title: "V1".into() };
    assert_eq!(find(Conditions { milestone: v1, ..Default::default() }), [2]);
    assert_eq!(find(Conditions { base: "main".into(), ..Default::default() }), [1, 2, 3]);
    assert!(find(Conditions { base: "Main".into(), ..Default::default() }).is_empty(), "branch names are exact");

    let mut drafts = prs.clone();
    drafts[0].draft = true;
    let only = |d: Draft| ids(matching_prs(&drafts, &Conditions { draft: d, ..Default::default() }, None));
    assert_eq!(only(Draft::Drafts), [1]);
    assert_eq!(only(Draft::Ready), [2, 3]);
    assert!(matching_prs(&prs, &Builtin::MyPrs.conditions(), None).is_none(), "waits for the signed-in user");
    assert!(needs_me(FilterKind::Prs, &Builtin::NeedsMyReview.conditions()));
    assert!(!needs_me(FilterKind::Prs, &Conditions { labels: vec!["x".into()], ..Default::default() }));
}

#[test]
fn team_requests_wait_for_members() {
    let team = |t: &str| vec![t.to_owned()];
    let facets = [
        PrFacets { author_id: "grace".into(), awaiting_team_ids: team("acme/core"), ..Default::default() },
        PrFacets { author_id: "grace".into(), awaiting_team_ids: team("acme/core"), reviewer_ids: team("ada"), ..Default::default() },
        PrFacets { author_id: "ada".into(), awaiting_team_ids: team("acme/core"), ..Default::default() },
        PrFacets { author_id: "grace".into(), awaiting_team_ids: team("acme/docs"), ..Default::default() },
        PrFacets { author_id: "grace".into(), awaiting_ids: team("ada"), reviewer_ids: team("ada"), ..Default::default() },
    ];
    let prs: Vec<PullRequest> = facets.iter().zip(1..).map(|(f, id)| pr(id, f.clone())).collect();
    let member = Me { teams: team("Acme/Core"), ..me() };
    let c = Builtin::NeedsMyReview.conditions();
    // 2 was reviewed by Ada already, 3 is her own and 4 asks another team; 5 asks her again.
    assert_eq!(ids(matching_prs(&prs, &c, Some(&member))), [1, 5], "team names ignore case");
    assert_eq!(ids(matching_prs(&prs, &c, Some(&me()))), [5], "no teams, personal requests only");
    assert!(super::matching::waiting_for(&facets[0], &member));
    assert!(!super::matching::waiting_for(&facets[1], &member));
    // A saved filter waiting for my review counts team requests too, with its other conditions.
    let labelled = PrFacets { labels: vec!["ui".into()], ..facets[0].clone() };
    let saved = Conditions { review: Review::WaitingForMe, labels: vec!["ui".into()], ..Default::default() };
    let prs = [pr(1, labelled), pr(2, facets[0].clone())];
    assert_eq!(ids(matching_prs(&prs, &saved, Some(&member))), [1], "only the one with the label");
}

#[test]
fn names_what_a_service_cannot_filter_by() {
    let reason = |p, kind, c: Conditions| unsupported(p, kind, &c).unwrap_or_default();
    let issues = FilterKind::Issues;
    assert!(reason(Provider::GitLab, issues, Builtin::IssuesMentioningMe.conditions()).contains("GitLab"));
    assert!(reason(Provider::JiraCloud, issues, Builtin::IssuesMentioningMe.conditions()).contains("mention"));
    let labels = Conditions { labels: vec!["x".into()], ..Default::default() };
    assert!(reason(Provider::Bitbucket, issues, labels.clone()).contains("label"));
    assert!(reason(Provider::BitbucketDataCenter, FilterKind::Prs, labels.clone()).contains("Bitbucket Data Center"));
    let milestone = Conditions { milestone: Milestone::None, ..Default::default() };
    assert!(reason(Provider::AzureDevOps, issues, milestone).contains("work items"));
    assert_eq!(
        reason(Provider::Bitbucket, FilterKind::Prs, Builtin::PrsAssignedToMe.conditions()),
        "Bitbucket can't filter pull requests by assignee."
    );
    for ok in [Builtin::AllPrs, Builtin::MyPrs, Builtin::NeedsMyReview] {
        assert_eq!(unsupported(Provider::Bitbucket, FilterKind::Prs, &ok.conditions()), None, "{ok:?}");
    }
    assert_eq!(unsupported(Provider::GitHub, issues, &Builtin::IssuesMentioningMe.conditions()), None);
    assert_eq!(unsupported(Provider::AzureDevOps, issues, &labels), None);
    let sprint = Conditions { sprint: Sprint::Open, ..Default::default() };
    assert_eq!(unsupported(Provider::JiraDataCenter, issues, &sprint), None);
    assert!(unsupported(Provider::GitHub, issues, &sprint).is_some());
    assert_eq!(reason(Provider::Trello, issues, Builtin::IssuesCreatedByMe.conditions()), "Trello can't filter cards by author.");
    assert!(reason(Provider::Trello, FilterKind::Prs, Builtin::MyPrs.conditions()).contains("doesn't host pull requests"));

    let comma = Conditions { labels: vec!["a,b".into()], ..Default::default() };
    assert_eq!(problems(Provider::GitHub, issues, &comma), ["GitHub can't search for a label that contains a comma: \u{201c}a,b\u{201d}."]);
    assert_eq!(problems(Provider::GitLabSelfManaged, FilterKind::Prs, &comma).len(), 1);
    assert!(problems(Provider::JiraCloud, issues, &comma).is_empty(), "Jira takes one label per clause");
    let empty = Conditions { labels: vec![" ".into()], ..Default::default() };
    assert_eq!(problems(Provider::GitHub, issues, &empty), ["A label is empty."]);
}

#[test]
fn issue_queries_per_service() {
    let c = |f: fn(&mut Conditions)| {
        let mut c = Conditions::default();
        f(&mut c);
        c
    };
    let gh = github::issue_params(
        &c(|c| {
            c.author = Who::Me;
            c.labels = vec!["good first".into(), "bug".into()];
            c.milestone = Milestone::Named { key: "4".into(), title: "v1".into() };
        }),
        "ada lovelace",
    );
    assert_eq!(gh, "state=open&per_page=100&sort=updated&creator=ada%20lovelace&labels=good%20first,bug&milestone=4");
    let gh = github::issue_params(
        &c(|c| {
            c.assignee = Who::Nobody;
            c.milestone = Milestone::None;
        }),
        "ada",
    );
    assert_eq!(gh, "state=open&per_page=100&sort=updated&assignee=none&milestone=none");
    assert!(github::issue_params(&Builtin::IssuesMentioningMe.conditions(), "ada").ends_with("&mentioned=ada"));

    let gl = gitlab::issue_params(
        &c(|c| {
            c.author = Who::Me;
            c.assignee = Who::Nobody;
            c.milestone = Milestone::None;
        }),
        "ada",
    );
    assert_eq!(gl, "state=opened&per_page=100&order_by=updated_at&scope=all&assignee_id=None&author_username=ada&milestone=None");

    let bb = bitbucket::cloud_q(
        &c(|c| {
            c.assignee = Who::Nobody;
            c.author = Who::Me;
            c.milestone = Milestone::Named { key: r#"v"1\"#.into(), title: r#"v"1\"#.into() };
        }),
        "{u-1}",
    );
    assert_eq!(bb, r#"(state="new" OR state="open") AND assignee = null AND reporter.uuid="{u-1}" AND milestone.name="v\"1\\""#);

    let az = azure::wiql_where(&c(|c| {
        c.assignee = Who::Me;
        c.mentions_me = true;
        c.labels = vec!["O'Brien".into()];
    }));
    assert!(az.contains("[System.AssignedTo] = @Me") && az.contains("[System.Id] IN (@RecentMentions)"), "{az}");
    assert!(az.ends_with("[System.Tags] CONTAINS 'O''Brien'"), "{az}");
    assert!(azure::wiql_where(&Builtin::IssuesCreatedByMe.conditions()).contains("[System.CreatedBy] = @Me"));

    let jira = jira::jql(
        &c(|c| {
            c.labels = vec![r#"say "hi""#.into()];
            c.sprint = Sprint::Open;
            c.milestone = Milestone::None;
        }),
        "PROJ",
    );
    assert_eq!(
        jira,
        r#"statusCategory != Done AND project = "PROJ" AND labels = "say \"hi\"" AND fixVersion is EMPTY AND sprint in openSprints() ORDER BY updated DESC"#
    );
    let jira = jira::jql(
        &c(|c| {
            c.author = Who::Me;
            c.sprint = Sprint::Named { id: 42, name: "S".into() };
        }),
        " ",
    );
    assert_eq!(jira, "statusCategory != Done AND reporter = currentUser() AND sprint = 42 ORDER BY updated DESC");
    assert_eq!(esc_quoted(r#"a"b\c'"#, '"'), r#"a\"b\\c'"#);
}
