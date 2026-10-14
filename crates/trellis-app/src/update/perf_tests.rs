//! Behaviour kept by the per-frame performance work: the bounded activity log.

use super::tests::state;
use crate::state::LOG_LIMIT;

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
