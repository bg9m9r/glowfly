//! A stash sub-list's drawn detail ("<date> · stash@{n}"), its hover text, and that the date is
//! no search keyword (the hidden keyword stays the bare ref).

use super::*;
use crate::state::stash_confirm::StashAction;
use git2::Oid;
use glowfly_core::settings::{Profile, ProfileId};

const NOW: i64 = 1_769_947_200;

fn settings() -> Settings {
    Settings { date_locale: "en_US".into(), relative_dates: true, ..Settings::default() }
}

fn stash(index: usize, message: &str, time: Option<i64>) -> StashInfo {
    StashInfo { index, message: message.into(), oid: Oid::from_bytes(&[index as u8 + 1; 20]).unwrap(), time }
}

/// Two stashes: 2 hours and 3 days old (the second one is `stash@{1}`).
fn dated() -> Vec<StashInfo> {
    vec![stash(0, "On main: newer", Some(NOW - 2 * 3600)), stash(1, "On main: older", Some(NOW - 3 * 86_400))]
}

#[test]
fn a_stash_row_shows_its_date_before_its_ref() {
    let s = settings();
    for pick in [Pick::stashes(StashAction::Apply, &dated()), Pick::stashes_to_rename(&dated())] {
        assert_eq!(pick.shown_detail(1, &s, NOW, 0).as_deref(), Some("3 days ago · stash@{1}"));
        assert_eq!(pick.shown_detail(0, &s, NOW, 0).as_deref(), Some("2 hours ago · stash@{0}"));
    }
    let pick = Pick::stashes(StashAction::Apply, &dated());
    let absolute = Settings { relative_dates: false, ..settings() };
    assert_eq!(pick.shown_detail(1, &absolute, NOW, 0), Some(format!("{} · stash@{{1}}", absolute.format_date(NOW - 3 * 86_400, 0))));
    let german = Settings { language: "de".into(), ..settings() };
    assert_eq!(pick.shown_detail(1, &german, NOW, 0).as_deref(), Some("vor 3 Tagen · stash@{1}"));
}

#[test]
fn a_stash_without_a_time_shows_only_its_ref() {
    let pick = Pick::stashes(StashAction::Pop, &[stash(0, "autostash", None)]);
    assert_eq!(pick.shown_detail(0, &settings(), NOW, 0).as_deref(), Some("stash@{0}"));
    assert_eq!(pick.detail_tip(0, &settings(), NOW, 0), None);
}

#[test]
fn the_detail_tip_is_the_full_date() {
    let pick = Pick::stashes(StashAction::Apply, &dated());
    let time = NOW - 3 * 86_400;
    let s = settings();
    let tip = pick.detail_tip(1, &s, NOW, 0).unwrap();
    assert_eq!(tip, s.date_tooltip(time, NOW, 0));
    assert_eq!(tip.lines().count(), 1);
    let absolute = Settings { relative_dates: false, ..settings() };
    let tip = pick.detail_tip(1, &absolute, NOW, 0).unwrap();
    assert_eq!(tip, absolute.date_tooltip(time, NOW, 0));
    assert_eq!(tip.lines().count(), 2, "{tip:?}");
    assert_eq!(tip.lines().nth(1), Some("3 days ago"));
}

#[test]
fn the_date_is_not_a_search_keyword() {
    let mut pick = Pick::stashes(StashAction::Apply, &dated());
    for i in 0..2 {
        assert_eq!(pick.detail(i), Some(format!("stash@{{{i}}}")));
    }
    pick.refresh("stash@{1}");
    let hits: Vec<usize> = pick.ranked().unwrap().hits.iter().map(|h| h.index).collect();
    assert_eq!(hits, vec![1]);
    // "days" is in no message, and the drawn "3 days ago" is not a keyword.
    assert_eq!(pick.rank("days").unwrap().total, 0);
    assert_eq!(pick.rank("hours").unwrap().total, 0);
}

#[test]
fn other_kinds_keep_their_detail() {
    let mut s = Settings::default();
    for (id, name, email) in [(1, "Work", "work@example.com"), (2, "Home", "")] {
        s.profiles.push(Profile { id: ProfileId(id), name: name.into(), author_email: email.into(), ..Profile::default() });
    }
    let pick = Pick::profiles(&s);
    let rows = pick.rank("").unwrap().total;
    assert_eq!(rows, 2);
    for i in 0..rows {
        assert_eq!(pick.shown_detail(i, &s, NOW, 0), pick.detail(i));
        assert_eq!(pick.detail_tip(i, &s, NOW, 0), None);
    }
}
