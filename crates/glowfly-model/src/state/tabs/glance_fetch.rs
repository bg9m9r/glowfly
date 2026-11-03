//! The status bar's fetch item (design pass D15): "Fetched 4 min ago", "Fetching…" while a job
//! that fetches runs in the active tab, "Never fetched" before the first; its tooltip lists each
//! remote with how long ago it was fetched (`Snapshot::fetched_remotes` names the remotes the
//! last fetch covered), and a click fetches. The label only changes on minute boundaries, so the
//! view repaints for it once a minute (`changes_in`).

use crate::state::AppState;
use crate::state::toolbar_tips::fetched_line;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::Settings;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// Whether a job that fetches runs in the active tab's repository.
pub fn fetching(st: &AppState) -> bool {
    let here = st.workdir();
    st.job.as_ref().is_some_and(|j| j.fetches && j.workdir.is_some() && j.workdir == here)
}

/// The item's text for a repository last fetched at `fetched_at` (Unix seconds).
pub fn label(t: Lang, settings: &Settings, fetched_at: Option<i64>, fetching: bool, now: i64, utc_offset: i32) -> String {
    if fetching {
        return t.tr("glance.fetching").to_owned();
    }
    let Some(at) = fetched_at else { return t.tr("glance.never_fetched").to_owned() };
    let age = (now - at).max(0);
    let n = |unit: i64| (age / unit) as usize;
    if age < MINUTE {
        t.tr("glance.fetched.just_now").to_owned()
    } else if age < HOUR {
        t.trn("glance.fetched.minutes", n(MINUTE), &[])
    } else if age < DAY {
        t.trn("glance.fetched.hours", n(HOUR), &[])
    } else if age <= 30 * DAY {
        t.trn("glance.fetched.days", n(DAY), &[])
    } else {
        t.trf("glance.fetched.on", &[("date", &settings.format_date_word(at, utc_offset))])
    }
}

/// The tooltip: each remote with how long ago it was fetched, then what a click does.
pub fn tip(t: Lang, settings: &Settings, snap: &Snapshot, now: i64, utc_offset: i32) -> String {
    let when = fetched_line(t, settings, snap.fetched_at, now, utc_offset);
    let mut lines: Vec<String> = snap
        .remote_info
        .iter()
        .map(|r| {
            let remote: &dyn std::fmt::Display = &r.name;
            // Before the first fetch every remote says so; after it, a remote the fetch left out.
            if snap.fetched_at.is_none() || snap.fetched_remotes.contains(&r.name) {
                t.trf("glance.fetch.remote", &[("remote", remote), ("when", &when)])
            } else {
                t.trf("glance.fetch.not_in_last", &[("remote", remote)])
            }
        })
        .collect();
    lines.push(t.tr("glance.fetch.tip").to_owned());
    lines.join("\n")
}

/// What a click on the item runs: the toolbar's Fetch, from every remote.
pub fn fetch_all() -> crate::msg::Msg {
    crate::msg::Msg::git("Fetch all", &["fetch", "--all", "--prune"])
}

/// How many seconds until `label` next changes: at the next whole minute of the fetch's age
/// (at least 1). None with no fetch time, whose label never changes by itself.
pub fn changes_in(fetched_at: Option<i64>, now: i64) -> Option<u64> {
    let age = (now - fetched_at?).max(0);
    Some((MINUTE - age % MINUTE).max(1) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::repo::RemoteInfo;

    const NOW: i64 = 1_800_000_000;

    fn label_at(age: Option<i64>, fetching: bool) -> String {
        label(Lang::En, &Settings::default(), age.map(|a| NOW - a), fetching, NOW, 0)
    }

    #[test]
    fn the_label_says_how_long_ago() {
        assert_eq!(label_at(None, false), "Never fetched");
        assert_eq!(label_at(Some(10), false), "Fetched just now");
        assert_eq!(label_at(Some(4 * MINUTE + 5), false), "Fetched 4 min ago");
        assert_eq!(label_at(Some(3 * HOUR), false), "Fetched 3 h ago");
        assert_eq!(label_at(Some(DAY), false), "Fetched 1 day ago");
        assert!(label_at(Some(40 * DAY), false).starts_with("Fetched "), "a date");
        assert_eq!(label_at(Some(10), true), "Fetching…");
        assert_eq!(label_at(None, true), "Fetching…", "before the first fetch too");
    }

    #[test]
    fn it_changes_on_the_minute() {
        assert_eq!(changes_in(None, NOW), None);
        assert_eq!(changes_in(Some(NOW - 10), NOW), Some(50));
        assert_eq!(changes_in(Some(NOW - 4 * MINUTE), NOW), Some(60));
        assert_eq!(changes_in(Some(NOW + 5), NOW), Some(60), "a time ahead of the clock counts as just now");
    }

    #[test]
    fn the_tip_lists_every_remote() {
        let remote = |name: &str| RemoteInfo { name: name.into(), ..RemoteInfo::default() };
        let mut snap = crate::state::op_fixture::snapshot("glance-fetch-tip");
        snap.remote_info = vec![remote("origin"), remote("upstream")];
        snap.fetched_at = None;
        let tip_now = |snap: &Snapshot| tip(Lang::En, &Settings::default(), snap, NOW, 0);
        assert_eq!(
            tip_now(&snap),
            "origin: Not fetched yet\nupstream: Not fetched yet\nClick to fetch from every remote.",
            "before the first fetch"
        );
        snap.fetched_at = Some(NOW - 2 * HOUR);
        snap.fetched_remotes = vec!["origin".into()];
        assert_eq!(tip_now(&snap), "origin: Last fetched 2 hours ago\nupstream: not in the last fetch\nClick to fetch from every remote.",);
    }
}
