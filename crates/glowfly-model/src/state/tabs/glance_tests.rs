//! Tests for the glance rules: the slot's order, the tab's description and the last failure.

use super::*;
use crate::state::logs::LogEntry;
use glowfly_core::logs::RunInfo;
use std::collections::HashSet;

fn run(ok: bool, title: &str) -> LogEntry {
    LogEntry { run: Some(RunInfo { title: title.into(), summary: String::new(), took_ms: None, routine: false }), ..LogEntry::new(ok, title) }
}

fn counts(changed: usize, behind: usize) -> Glance {
    Glance { changed, behind, upstream: Some("origin/main".into()), ..Glance::default() }
}

#[test]
fn the_slot_shows_the_first_state_that_holds() {
    let failed = Some(Failure { label: "Fetch".into(), entry: EntryId::next() });
    let all = TabGlance {
        running: Some("Push".into()),
        failed: failed.clone(),
        counts: Some(Glance { op: Some(RepositoryState::Merge), ..counts(3, 2) }),
    };
    assert_eq!(all.mark(), Some(Mark::Running));
    let g = TabGlance { running: None, ..all.clone() };
    assert_eq!(g.mark(), Some(Mark::Failed));
    let g = TabGlance { failed: None, ..g };
    assert_eq!(g.mark(), Some(Mark::InProgress));
    let conflicts = TabGlance { counts: Some(Glance { conflicted: 1, ..counts(3, 2) }), ..TabGlance::default() };
    assert_eq!(conflicts.mark(), Some(Mark::InProgress), "conflicts left without an operation");
    let g = TabGlance { counts: Some(counts(3, 2)), ..TabGlance::default() };
    assert_eq!(g.mark(), Some(Mark::Changed));
    let g = TabGlance { counts: Some(counts(0, 2)), ..TabGlance::default() };
    assert_eq!(g.mark(), Some(Mark::Behind(2)));
    let g = TabGlance { counts: Some(Glance { ahead: 4, ..counts(0, 0) }), ..TabGlance::default() };
    assert_eq!(g.mark(), None, "ahead has no mark");
    assert_eq!(TabGlance::default().mark(), None);
}

#[test]
fn the_description_lists_every_state() {
    let t = Lang::En;
    let g = TabGlance { counts: Some(counts(3, 2)), ..TabGlance::default() };
    assert_eq!(g.describe(t, "api-server"), "api-server: 3 changed files, 2 behind origin/main");
    let all = TabGlance {
        running: Some("Fetch all".into()),
        failed: Some(Failure { label: "Push".into(), entry: EntryId::next() }),
        counts: Some(Glance { op: Some(RepositoryState::Merge), conflicted: 1, ahead: 1, ..counts(1, 0) }),
    };
    let text = all.describe(t, "web");
    assert!(text.starts_with("web: "), "{text}");
    for part in ["running", "Push failed", "Merge in progress", "1 conflicted file left", "1 changed file", "1 ahead of origin/main"] {
        assert!(text.contains(part), "{part} in {text}");
    }
    assert_eq!(TabGlance::default().describe(t, "docs"), "docs", "nothing to say");
}

#[test]
fn the_last_failure_is_the_newest_run_when_it_failed() {
    let none = HashSet::new();
    assert_eq!(last_failure(&[], &none), None);
    let log = vec![run(true, "Pull"), run(false, "Fetch all"), LogEntry::new(true, "a note")];
    let f = last_failure(&log, &none).expect("notes after it don't count");
    assert_eq!((f.label.as_str(), f.entry), ("Fetch all", log[1].id));
    let mut later = log.clone();
    later.push(run(true, "Fetch all"));
    assert_eq!(last_failure(&later, &none), None, "a later success clears it");
    let dismissed: HashSet<_> = [log[1].id].into();
    assert_eq!(last_failure(&log, &dismissed), None);
    let mut earlier = run(false, "Push");
    earlier.earlier = Some(crate::state::logs::Earlier { session: 1, at: 1 });
    assert_eq!(last_failure(&[earlier], &none), None, "an earlier session's");
}
