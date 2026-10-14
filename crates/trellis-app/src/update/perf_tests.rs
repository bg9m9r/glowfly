//! Behaviour kept by the per-frame performance work: the bounded activity log and the
//! allocation-free commit search.

use super::commit_matches;
use super::tests::state;
use crate::state::LOG_LIMIT;
use git2::Oid;
use trellis_core::repo::CommitInfo;

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
    assert!(st.view.show_log);
}

#[test]
fn commit_search_ignores_case_and_matches_id_prefixes() {
    let c = CommitInfo {
        oid: Oid::from_str("0abcdef1234567890abcdef1234567890abcdef1").unwrap(),
        summary: "Fix the Parser".into(),
        body: "Über-long BODY text".into(),
        author: "Ada Lovelace".into(),
        email: "ada@example.com".into(),
        time: 0,
        parents: Vec::new(),
    };
    for q in ["parser", "fix the", "über-long", "body", "lovelace", "0abc", "0abcdef1234567890abcdef1234567890abcdef1"] {
        assert!(commit_matches(&c, q), "{q}");
    }
    for q in ["parsers", "abcdef", "ada@", "0abcdef1234567890abcdef1234567890abcdef12"] {
        assert!(!commit_matches(&c, q), "{q}");
    }
}
