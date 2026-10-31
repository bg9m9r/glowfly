//! Behaviour kept by the per-frame performance work: the bounded activity log and the
//! allocation-free commit search.

use super::tests::state;
use crate::msg::RepoMsg;
use crate::state::LOG_LIMIT;
use git2::Oid;
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::repo::CommitInfo;

#[test]
fn activity_log_keeps_only_the_newest_entries() {
    let mut st = state();
    for i in 0..LOG_LIMIT + 120 {
        st.log_quiet(true, format!("fetch {i}"));
    }
    assert_eq!(st.log.len(), LOG_LIMIT);
    assert_eq!(st.log[0].text, "fetch 120");
    assert_eq!(st.log.last().unwrap().text, format!("fetch {}", LOG_LIMIT + 119));
    assert!(!st.view.show_log, "a quiet entry doesn't open the log");
    st.log(false, "failed");
    assert_eq!(st.log.len(), LOG_LIMIT);
    assert!(st.logs.announce.is_some() && !st.view.show_log, "announced at the end of the next update");
}

#[test]
fn commit_search_ignores_case_and_matches_id_prefixes() {
    let c = CommitInfo {
        oid: Oid::from_str("0abcdef1234567890abcdef1234567890abcdef1").unwrap(),
        summary: "Fix the Parser".into(),
        body: "Über-long BODY text".into(),
        author: glowfly_core::repo::Author::new("Ada Lovelace", "ada@example.com"),
        time: 0,
        parents: Default::default(),
    };
    for q in ["parser", "fix the", "über-long", "body", "lovelace", "0abc", "0abcdef1234567890abcdef1234567890abcdef1"] {
        assert!(CommitQuery::parse(q).matches(&c), "{q}");
    }
    for q in ["parsers", "abcdef", "ada@", "0abcdef1234567890abcdef1234567890abcdef12"] {
        assert!(!CommitQuery::parse(q).matches(&c), "{q}");
    }
}

#[test]
fn dropped_stats_can_be_asked_for_again() {
    use super::tests::{drive, scratch_repo};
    use crate::effect::Effect;
    use crate::msg::Msg;
    let dir = scratch_repo("stats-dropped");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let oid = st.snap.as_ref().unwrap().commits[0].oid;
    let now = std::time::Instant::now();
    let asks = |fx: &[Effect]| fx.iter().any(|e| matches!(e, Effect::Stats { oids, .. } if *oids == [oid]));
    assert!(asks(&super::update(&mut st, Msg::Repo(RepoMsg::WantStats(vec![oid])), now)));
    assert!(!asks(&super::update(&mut st, Msg::Repo(RepoMsg::WantStats(vec![oid])), now)), "already asked");
    super::update(&mut st, Msg::Repo(RepoMsg::StatsDropped(vec![oid])), now);
    assert!(asks(&super::update(&mut st, Msg::Repo(RepoMsg::WantStats(vec![oid])), now)), "a newer request had replaced it");
    let _ = std::fs::remove_dir_all(&dir);
}
