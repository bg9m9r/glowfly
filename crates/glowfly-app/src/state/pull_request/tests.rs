use super::*;
use glowfly_core::forge::{MergeState, PrDetail, Provider, Reviewer, Target};

pub(crate) fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

pub(crate) fn pr(head: Option<&str>) -> PullRequest {
    PullRequest {
        id: 7,
        title: "Add search".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: head.map(str::to_owned),
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

pub(crate) fn view(pr: PullRequest, tips: Tips) -> PrView {
    PrView {
        pr,
        label: "#7".into(),
        noun: "pull request",
        remote: "origin".into(),
        listed: true,
        tips,
        files: Files::Missing,
        head_ref: None,
        shown: None,
        head_fetch: Default::default(),
        asked: None,
        tab: PrTab::Overview,
        page: Page::Loading,
        reloading: false,
        busy: None,
        action_error: None,
        forms: Default::default(),
        threads: Default::default(),
        checks: crate::state::pull_request_checks::PrChecks::new(&crate::state::ForgeState::new(None, String::new(), None)),
        line_draft: None,
        lines: Default::default(),
    }
}

fn reviewer(name: &str, state: &str) -> Reviewer {
    Reviewer { name: name.into(), state: state.into(), required: false }
}

#[test]
fn tips_come_from_the_named_remote_only() {
    let remotes = vec![
        ("up".to_owned(), vec![("main".to_owned(), oid(1))]),
        ("up/stream".to_owned(), vec![("main".to_owned(), oid(2)), ("feature".to_owned(), oid(3))]),
    ];
    let p = pr(None);
    assert_eq!(Tips::in_remotes(&remotes, "up", &p), Tips { target: Some(oid(1)), source: None });
    assert_eq!(Tips::in_remotes(&remotes, "up/stream", &p), Tips { target: Some(oid(2)), source: Some(oid(3)) });
    assert_eq!(Tips::in_remotes(&remotes, "gone", &p), Tips { target: None, source: None });
}

#[test]
fn headings_and_summaries() {
    assert_eq!(files_heading(1), "1 file changed");
    assert_eq!(files_heading(3), "3 files changed");
    assert_eq!(files_heading(0), "0 files changed");
    let mut p = pr(None);
    assert_eq!(review_summary(&p), "No reviewers");
    p.reviewers = vec![reviewer("a", "approved"), reviewer("b", "approved"), reviewer("c", "waiting")];
    assert_eq!(review_summary(&p), "2 of 3 approved");
}

#[test]
fn reviewer_tones() {
    assert_eq!(reviewer_tone("approved"), Tone::Good);
    assert_eq!(reviewer_tone("Approved with suggestions"), Tone::Good);
    assert_eq!(reviewer_tone("changes requested"), Tone::Bad);
    assert_eq!(reviewer_tone("rejected"), Tone::Bad);
    assert_eq!(reviewer_tone("waiting"), Tone::Neutral);
    assert_eq!(reviewer_tone("no vote"), Tone::Neutral);
}

#[test]
fn descriptions_keep_lines_and_drop_controls() {
    assert_eq!(description_text("one\n\ttwo\u{202E}\u{7}\r"), "one\n\ttwo");
}

#[test]
fn files_texts_name_the_branches() {
    let v = view(pr(None), Tips { target: Some(oid(1)), source: Some(oid(2)) });
    assert_eq!(empty_text(&v), "No files differ: feature has nothing that main doesn't.");
    assert!(!unrelated_text(&v, false).contains("shallow"));
    assert!(unrelated_text(&v, true).ends_with("This is a shallow clone: fetching more history may find it."));
}

#[test]
fn only_services_with_in_app_reviews_load_a_page() {
    let fs = |provider| {
        let t = Target { provider, host: "example.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
        ForgeState::new(Some(t), "origin".into(), None)
    };
    assert_eq!(initial_page(&fs(Provider::GitHub)), Page::Loading);
    assert_eq!(initial_page(&fs(Provider::GitHubEnterprise)), Page::Loading);
    assert_eq!(initial_page(&fs(Provider::GitLab)), Page::Unsupported);
    assert_eq!(initial_page(&fs(Provider::AzureDevOps)), Page::Unsupported);
    assert_eq!(initial_page(&ForgeState::new(None, "origin".into(), None)), Page::Unsupported);
}

pub(crate) fn merged_detail() -> PrDetail {
    PrDetail {
        state: PrState::Merged,
        draft: false,
        locked: false,
        mergeable: None,
        merge_state: MergeState::Unknown,
        head_sha: String::new(),
        base_sha: String::new(),
        same_repo: true,
        author: "ada".into(),
        me: "grace".into(),
        can_merge: Some(true),
        methods: Vec::new(),
        deletes_merged_branches: false,
        pending_review: None,
    }
}
