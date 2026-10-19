//! A successful fetch or pull stamps the snapshot's fetch time (Pull's tooltip footer); a failed
//! one, or a job that doesn't fetch, leaves it alone.

use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::update::update;
use std::time::{Duration, Instant};

fn done(st: &mut AppState, ok: bool) {
    update(st, Msg::Job(JobMsg::JobDone { ok, output: String::new(), undo: None }), Instant::now());
}

fn fetched(st: &AppState) -> Option<i64> {
    st.snap.as_ref().unwrap().fetched_at
}

#[test]
fn fetches_stamp_the_time_and_other_jobs_do_not() {
    let dir = scratch_repo("fetch-time");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(fetched(&st), None, "never fetched");
    st.view.now = 1_769_947_200;

    update(&mut st, Msg::git("Fetch all", &["fetch", "--all", "--prune"]), Instant::now());
    done(&mut st, false);
    assert_eq!(fetched(&st), None, "a failed fetch");

    update(&mut st, Msg::git("Push", &["push"]), Instant::now());
    done(&mut st, true);
    assert_eq!(fetched(&st), None, "Push doesn't fetch");

    update(&mut st, Msg::git("Fetch all", &["fetch", "--all", "--prune"]), Instant::now());
    done(&mut st, true);
    assert_eq!(fetched(&st), Some(1_769_947_200));

    // The quiet background fetch counts too.
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.snap.as_mut().unwrap().fetched_at = None;
    st.settings.auto_fetch_minutes = 1;
    let due = st.last_fetch + Duration::from_secs(61);
    let fx = update(&mut st, Msg::Tick(due), Instant::now());
    assert_eq!(fx.len(), 1, "the background fetch starts");
    st.view.now += 60;
    done(&mut st, true);
    assert_eq!(fetched(&st), Some(1_769_947_260));
    let _ = std::fs::remove_dir_all(&dir);
}
