//! A sub-list's matching: building a Pick for its kind, loading items, ranking the query and the
//! selected item (the Pick type is in palette.rs).

use super::profiles::profile_rows;
use super::{Entry, Hit, IssuePickRow, Items, MAX_PICK_ROWS, PaletteCommand, Pick, PickKind, Ranked, StashAction, issues_to_view};
use crate::state::forge::ForgeState;
use glowfly_core::fuzzy;
use glowfly_core::repo::StashInfo;
use glowfly_core::settings::Settings;

impl Pick {
    pub fn new(kind: PickKind) -> Self {
        Self { kind, items: Items::Loading, stashes: Vec::new(), profiles: Vec::new(), issues: Vec::new(), refs: Vec::new(), ranked: None }
    }

    /// A stash sub-list over `list`, labelled by the stash messages.
    pub fn stashes(action: StashAction, list: &[StashInfo]) -> Self {
        Self::over_stashes(PickKind::Stash(action), list)
    }

    /// The stash sub-list for renaming over `list`, the same rows as `stashes`.
    pub fn stashes_to_rename(list: &[StashInfo]) -> Self {
        Self::over_stashes(PickKind::RenameStash, list)
    }

    fn over_stashes(kind: PickKind, list: &[StashInfo]) -> Self {
        let items = Items::Ready(list.iter().map(|s| s.message.clone()).collect());
        Self { items, stashes: list.to_vec(), ..Self::new(kind) }
    }

    /// The profile sub-list, labelled by the profile names in settings order.
    pub fn profiles(settings: &Settings) -> Self {
        let (names, profiles) = profile_rows(settings);
        Self { items: Items::Ready(names), profiles, ..Self::new(PickKind::Profile) }
    }

    /// The issue sub-list for starting a `kind` flow branch, over the issues assigned to the
    /// user, labelled "<mention>  <title>".
    pub fn issues(kind: glowfly_core::gitflow::FlowKind, f: &ForgeState) -> Self {
        let labels = f.my_issues.iter().map(|i| format!("{}  {}", f.mention(&i.key), i.title)).collect();
        let issues = f.my_issues.iter().map(IssuePickRow::of).collect();
        Self { items: Items::Ready(labels), issues, ..Self::new(PickKind::GitFlowIssue(kind)) }
    }

    /// The issue sub-list for opening the issue panel, over the listed issues (`issues_to_view`),
    /// labelled "<mention>  <title>".
    pub fn issues_to_view(f: &ForgeState) -> Self {
        let listed = issues_to_view(f);
        let labels = listed.iter().map(|i| format!("{}  {}", f.mention(&i.key), i.title)).collect();
        let issues = listed.into_iter().map(IssuePickRow::of).collect();
        Self { items: Items::Ready(labels), issues, ..Self::new(PickKind::ViewIssue) }
    }

    /// The Switch branch sub-list: the main list's checkout entries for `snap` (local branches,
    /// those another worktree has, then remote branches and tags), labelled without their
    /// leading "Checkout ".
    pub fn checkouts(snap: &glowfly_core::repo::Snapshot) -> Self {
        let refs = super::commands::ref_entries(Some(snap));
        let checkouts: Vec<Entry> = refs.locals.into_iter().chain(refs.held).chain(refs.others).collect();
        let labels = checkouts.iter().map(|e| e.label.strip_prefix("Checkout ").unwrap_or(&e.label).to_owned()).collect();
        Self { items: Items::Ready(labels), refs: checkouts, ..Self::new(PickKind::SwitchBranch) }
    }

    /// The Delete tag sub-list: one row per tag of `snap` (tags on a tree or blob aren't
    /// listed), with the tagged commit's short sha as its detail.
    pub fn tags_to_delete(snap: &glowfly_core::repo::Snapshot) -> Self {
        use super::commands::{entry, with_detail};
        let refs: Vec<Entry> = snap
            .tags
            .iter()
            .map(|(name, oid)| {
                let e = entry(PaletteCommand::DeleteTag(name.clone()), name.clone(), "");
                with_detail(e, Some(oid.to_string()[..7].to_owned()))
            })
            .collect();
        let labels = snap.tags.iter().map(|(n, _)| n.clone()).collect();
        Self { items: Items::Ready(labels), refs, ..Self::new(PickKind::DeleteTag) }
    }

    /// The dim text at the right of item `index`: a stash's ref, a profile's email, or what a
    /// checkout does.
    pub fn detail(&self, index: usize) -> Option<String> {
        match self.kind {
            PickKind::Stash(_) | PickKind::RenameStash => self.stashes.get(index).map(|s| format!("stash@{{{}}}", s.index)),
            PickKind::Profile => self.profiles.get(index).map(|r| r.detail.clone()),
            PickKind::GitFlowIssue(_) | PickKind::ViewIssue => self.issues.get(index).map(|r| r.detail.clone()),
            PickKind::SwitchBranch | PickKind::DeleteTag => self.refs.get(index).and_then(|e| e.detail.clone()),
            PickKind::FileHistory | PickKind::Blame => None,
        }
    }

    /// What choosing item `index` runs.
    pub fn command(&self, index: usize) -> Option<PaletteCommand> {
        match self.kind {
            PickKind::FileHistory => Some(match super::files::folder_of(self.label(index)) {
                Some(folder) => PaletteCommand::OpenFolderHistory(folder.to_owned()),
                None => PaletteCommand::OpenFileHistory(self.label(index).to_owned()),
            }),
            PickKind::Blame => Some(PaletteCommand::OpenBlame(self.label(index).to_owned())),
            PickKind::Stash(action) => self.stashes.get(index).map(|s| PaletteCommand::StashOp { action, oid: s.oid }),
            PickKind::RenameStash => self.stashes.get(index).map(|s| PaletteCommand::RenameStash { oid: s.oid }),
            PickKind::Profile => self.profiles.get(index).map(|r| PaletteCommand::SwitchProfile(r.id)),
            PickKind::GitFlowIssue(kind) => {
                self.issues.get(index).map(|r| PaletteCommand::StartGitFlowFromIssue { kind, name: r.name.clone() })
            }
            PickKind::ViewIssue => self.issues.get(index).map(|r| PaletteCommand::ViewIssue(r.key.clone())),
            PickKind::SwitchBranch | PickKind::DeleteTag => self.refs.get(index).map(|e| e.command.clone()),
        }
    }

    pub fn set_items(&mut self, items: Items) {
        self.items = items;
        self.ranked = None;
    }

    /// The ranking from the last `refresh`; None while the items aren't ready.
    pub fn ranked(&self) -> Option<&Ranked> {
        self.ranked.as_deref()
    }

    /// The text of item `index`.
    pub fn label(&self, index: usize) -> &str {
        match &self.items {
            Items::Ready(v) => v.get(index).map_or("", String::as_str),
            _ => "",
        }
    }

    /// Ranks the items for `query`, unless they already are. A query that extends the last one
    /// only searches the last matches: whatever matches the longer query matches the shorter.
    /// Details (a stash's ref, a profile's email) match as hidden keywords.
    pub fn refresh(&mut self, query: &str) {
        if self.ranked.as_ref().is_some_and(|r| r.query == query) {
            return;
        }
        if let Some(ranked) = self.rank(query) {
            self.ranked = Some(Box::new(ranked));
        }
    }

    /// The items ranked for `query` as `refresh` would, without keeping it: for a query typed
    /// this frame that update hasn't ranked yet. None while the items aren't ready.
    pub fn rank(&self, query: &str) -> Option<Ranked> {
        let Items::Ready(items) = &self.items else { return None };
        // One per item, so a detail can't land on another item (none at all for files).
        let details: Vec<String> = match self.kind {
            PickKind::FileHistory | PickKind::Blame => Vec::new(),
            _ => (0..items.len()).map(|i| self.detail(i).unwrap_or_default()).collect(),
        };
        let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        if squash(query).is_empty() {
            let hits = (0..items.len().min(MAX_PICK_ROWS)).map(|index| Hit { index, matched: Vec::new() }).collect();
            return Some(Ranked { query: query.to_owned(), matches: (0..items.len()).collect(), hits, total: items.len() });
        }
        let candidates: Vec<usize> = match self.ranked.as_deref() {
            Some(r) if !squash(&r.query).is_empty() && squash(query).starts_with(&squash(&r.query)) => {
                let mut m = r.matches.clone();
                // Back to item order, so ties keep ranking the earlier item first.
                m.sort_unstable();
                m
            }
            _ => (0..items.len()).collect(),
        };
        let keywords = |i: usize| details.get(i).map_or("", String::as_str);
        let pairs: Vec<(&str, &str)> = candidates.iter().map(|&i| (items[i].as_str(), keywords(i))).collect();
        let found = fuzzy::rank(query, &pairs);
        let total = found.len();
        let mut matches = Vec::with_capacity(total);
        let mut hits = Vec::new();
        for (pos, m) in found {
            let index = candidates[pos];
            matches.push(index);
            if hits.len() < MAX_PICK_ROWS {
                hits.push(Hit { index, matched: m.positions });
            }
        }
        Some(Ranked { query: query.to_owned(), matches, hits, total })
    }
}
