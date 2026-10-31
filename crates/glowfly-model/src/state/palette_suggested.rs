//! The palette's suggested commands for an empty query: what the repository's state makes
//! likely next (continue a stopped rebase, pull when behind, push when ahead, stage the
//! changes, branch, fetch), or opening a repository when none is open; and the empty query's
//! list split into its Recent, Suggested and All commands groups (`grouped`).

use super::AppState;
use super::op_steps::conflicted;
use super::palette::Group;
use super::palette_commands::palette_commands_rebase::RebaseCommand;
use super::palette_commands::{Entry, PaletteCommand};
use glowfly_core::fuzzy;
use glowfly_core::rebase::Resume;

/// Most commands the Suggested group shows.
const MAX_SUGGESTED: usize = 5;

/// Where `cmd` goes among the suggestions now (0 first), or None when it isn't suggested.
fn suggestion_rank(st: &AppState, cmd: &PaletteCommand) -> Option<usize> {
    use PaletteCommand as C;
    let Some(snap) = &st.snap else { return matches!(cmd, C::OpenRepo).then_some(0) };
    let head = snap.head_info();
    let has_remote = !snap.remote_info.is_empty() || !snap.remote_urls.is_empty();
    let rule = match cmd {
        C::Rebase(RebaseCommand::Resume(Resume::Continue)) => conflicted(snap) == 0,
        C::Pull => head.is_some_and(|b| b.upstream.is_some() && b.behind > 0),
        C::Push => head.is_some_and(|b| if b.upstream.is_some() { b.ahead > 0 } else { has_remote }),
        C::StageAll => !snap.unstaged.is_empty(),
        C::CreateBranch => true,
        C::Fetch => has_remote,
        _ => return None,
    };
    let rank = match cmd {
        C::Rebase(_) => 0,
        C::Pull => 1,
        C::Push => 2,
        C::StageAll => 3,
        C::CreateBranch => 4,
        _ => 5,
    };
    rule.then_some(rank)
}

/// A ranked row: its index into the entries and its match.
type Ranked = (usize, fuzzy::Match);

/// The empty query's list (`ranked`: the recent commands first, then the rest in entry order)
/// split into Recent, Suggested and All commands, each command once, with the groups' first
/// rows. No groups when only one would have rows.
pub(super) fn grouped(st: &AppState, all: &[Option<Entry>], ranked: Vec<Ranked>) -> (Vec<Ranked>, Vec<(Group, usize)>) {
    let command = |i: usize| all[i].as_ref().map(|e| &e.command);
    let is_recent = |i: usize| command(i).and_then(PaletteCommand::recall_key).is_some_and(|k| st.palette_recent.position(k).is_some());
    let (recent, rest): (Vec<_>, Vec<_>) = ranked.into_iter().partition(|(i, _)| is_recent(*i));
    let rank = |i: usize| command(i).and_then(|c| suggestion_rank(st, c));
    let mut suggested: Vec<_> = rest.iter().enumerate().filter_map(|(at, (i, _))| Some((rank(*i)?, at))).collect();
    suggested.sort_by_key(|&(r, _)| r);
    suggested.truncate(MAX_SUGGESTED);
    let picked: Vec<usize> = suggested.iter().map(|&(_, at)| at).collect();
    let mut slots: Vec<Option<Ranked>> = rest.into_iter().map(Some).collect();
    let suggested: Vec<_> = picked.iter().filter_map(|&at| slots[at].take()).collect();
    let others: Vec<_> = slots.into_iter().flatten().collect();
    let mut groups = Vec::new();
    let mut first = 0;
    for (g, n) in [(Group::Recent, recent.len()), (Group::Suggested, suggested.len()), (Group::All, others.len())] {
        if n > 0 {
            groups.push((g, first));
            first += n;
        }
    }
    if groups.len() < 2 {
        groups.clear();
    }
    (recent.into_iter().chain(suggested).chain(others).collect(), groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::repo::{FileEntry, RemoteInfo};
    use glowfly_core::settings::Settings;
    use std::time::Instant;

    #[test]
    fn suggestions_follow_the_state() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        assert_eq!(suggestion_rank(&st, &PaletteCommand::OpenRepo), Some(0), "no repository open");
        assert_eq!(suggestion_rank(&st, &PaletteCommand::Fetch), None);

        let snap = crate::state::op_fixture::snapshot("suggested");
        let dir = snap.workdir.clone();
        st.snap = Some(snap);
        let rank = |st: &AppState, c: PaletteCommand| suggestion_rank(st, &c);
        assert_eq!(rank(&st, PaletteCommand::OpenRepo), None, "a repository is open");
        assert_eq!(rank(&st, PaletteCommand::CreateBranch), Some(4));
        assert_eq!(rank(&st, PaletteCommand::Fetch), None, "no remotes");
        assert_eq!(rank(&st, PaletteCommand::Push), None, "nowhere to push");
        assert_eq!(rank(&st, PaletteCommand::StageAll), None, "nothing to stage");

        let s = st.snap.as_mut().unwrap();
        s.remote_info = vec![RemoteInfo { name: "origin".into(), urls: vec!["u".into()], ..RemoteInfo::default() }];
        s.unstaged = vec![FileEntry::new("a.txt", 'M')];
        assert_eq!(rank(&st, PaletteCommand::Fetch), Some(5));
        assert_eq!(rank(&st, PaletteCommand::Push), Some(2), "no upstream yet, and a remote");
        assert_eq!(rank(&st, PaletteCommand::StageAll), Some(3));

        let s = st.snap.as_mut().unwrap();
        let head = s.locals.iter_mut().find(|b| b.is_head).unwrap();
        head.upstream = Some("origin/main".into());
        head.behind = 2;
        assert_eq!(rank(&st, PaletteCommand::Pull), Some(1), "behind");
        assert_eq!(rank(&st, PaletteCommand::Push), None, "not ahead");
        st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.is_head).unwrap().ahead = 1;
        assert_eq!(rank(&st, PaletteCommand::Push), Some(2), "ahead");

        let cont = PaletteCommand::Rebase(RebaseCommand::Resume(Resume::Continue));
        assert_eq!(rank(&st, cont.clone()), Some(0));
        st.snap.as_mut().unwrap().unstaged = vec![FileEntry::new("a.txt", '!')];
        assert_eq!(rank(&st, cont), None, "conflicts remain");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_group_alone_has_no_headings() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        let snap = crate::state::op_fixture::snapshot("suggested-none");
        let dir = snap.workdir.clone();
        st.snap = Some(snap);
        let all: Vec<Option<Entry>> = super::super::palette_commands::entries(&st)
            .into_iter()
            .filter(|e| suggestion_rank(&st, &e.command).is_none())
            .map(Some)
            .collect();
        let ranked = (0..all.len()).map(|i| (i, fuzzy::Match { score: 0, positions: Vec::new() })).collect();
        let (rows, groups) = grouped(&st, &all, ranked);
        assert!(groups.is_empty(), "no recents and no suggestions: no headings");
        assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), (0..all.len()).collect::<Vec<_>>(), "entry order");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
