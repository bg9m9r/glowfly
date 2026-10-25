//! Headless tests for the dashboard's Review requested: requests made to a team the signed-in
//! user belongs to, and the teams kept with the user across refreshes.

use super::super::tests::state;
use super::tests::{deliver, fetch_of, gh, pr, reply, run, scanned, token};
use crate::effect::{AccountReply, TargetReply};
use crate::msg::DashboardMsg;
use crate::state::AppState;
use crate::state::dashboard_rows::counts;
use glowfly_core::forge::{Me, Provider, PullRequest};

/// The widgets repository on GitHub, read as `me`: a team request by Grace, the same team
/// request on Ada's own pull request, and Grace asking Ada herself.
fn filled_as(st: &mut AppState, me: Me) {
    st.dashboard.generation = 1;
    deliver(st, vec![scanned("widgets", &[("origin", "https://github.com/acme/widgets")], &[])]);
    let team = |mut p: PullRequest| {
        p.facets.awaiting_team_ids = vec!["acme/core".into()];
        p
    };
    let prs = vec![team(pr(1, "Speed up search", "grace", &[])), team(pr(2, "Fix login", "ada", &[])), pr(3, "Docs", "grace", &["ada"])];
    let github = AccountReply {
        account: gh("acme", "widgets").account(),
        auth: Ok(token(Provider::GitHub)),
        me: Some(Ok(me)),
        targets: vec![TargetReply { target: gh("acme", "widgets"), prs: Some(Ok(prs)), issues: Some(Ok(Vec::new())) }],
    };
    run(st, reply(1, false, vec![github]));
}

fn ada(teams: &[&str]) -> Me {
    Me { id: "ada".into(), name: "Ada".into(), teams: teams.iter().map(|t| t.to_string()).collect() }
}

#[test]
fn review_requested_counts_team_requests() {
    let mut st = state();
    filled_as(&mut st, ada(&["acme/core"]));
    let c = counts(&st.dashboard, "");
    assert_eq!((c[0], c[1]), (1, 2), "the team request counts, but not on Ada's own pull request");

    // A refresh hands the user back with their teams, so they aren't asked again.
    let generation = st.dashboard.generation;
    run(&mut st, DashboardMsg::Refresh);
    assert_eq!(st.dashboard.generation, generation + 1);
    let job = fetch_of(deliver(&mut st, vec![scanned("widgets", &[("origin", "https://github.com/acme/widgets")], &[])])).unwrap();
    assert_eq!(job.accounts[0].me.as_ref().map(|m| m.teams.clone()), Some(vec!["acme/core".to_owned()]));

    // Without teams (or a token that can't read them) only the personal request counts.
    let mut st = state();
    filled_as(&mut st, ada(&[]));
    assert_eq!(counts(&st.dashboard, "")[1], 1);
}
