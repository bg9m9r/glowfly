//! What the PULL REQUESTS and ISSUES sections show under their filters: the signed-in user
//! (looked up once per service state), the latest issue search, the filter dialog's options, and
//! the rows and hint each section draws. Pure: the views and `update` both read it.

use super::{ForgeState, Status};
use glowfly_core::forge::filters::{Named, matching};
use glowfly_core::forge::{Conditions, FilterChoice, FilterKind, FilterOptions, Issue, IssueFacets, Me, Provider, PullRequest, Target};
use glowfly_core::settings::RepoForgeFilters;

#[derive(Default)]
pub struct FilterState {
    pub me: MeState,
    /// The latest search for a filter other than "Assigned to me" (which is `my_issues`).
    pub issues: Option<IssueList>,
    pub pr_options: OptionsState,
    pub issue_options: OptionsState,
    /// Bumped when the account behind the code host changes, so a lookup of the user asked
    /// under the old one is dropped.
    pub host_epoch: u32,
    /// The same for the tracker's account and its issue searches.
    pub tracker_epoch: u32,
}

/// The signed-in user on the code host.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum MeState {
    #[default]
    Unknown,
    Asking,
    Known(Me),
    /// The lookup failed; the next refresh asks again.
    Failed(String),
}

pub struct IssueList {
    pub choice: FilterChoice,
    pub conditions: Conditions,
    /// The last answer's issues, kept while a refresh's search loads.
    pub items: Vec<Issue>,
    /// `Idle` while the search is in flight.
    pub status: Status,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum OptionsState {
    #[default]
    NotAsked,
    Loading,
    Loaded(FilterOptions),
    Failed(String),
}

impl FilterState {
    /// Replies in flight were dropped (the tab was parked): ask again later.
    pub fn replies_dropped(&mut self) {
        if self.me == MeState::Asking {
            self.me = MeState::Unknown;
        }
        for o in [&mut self.pr_options, &mut self.issue_options] {
            if *o == OptionsState::Loading {
                *o = OptionsState::NotAsked;
            }
        }
        // An issue list still loading stays `Idle`; the next refresh searches again.
    }

    /// The account behind the code host (`host`) and/or the tracker (`tracker`) was connected
    /// again or disconnected: forget what was learned under it and drop answers still on the way.
    pub fn account_changed(&mut self, host: bool, tracker: bool) {
        if host {
            self.host_epoch = self.host_epoch.wrapping_add(1);
            self.me = MeState::Unknown;
            self.pr_options = OptionsState::NotAsked;
        }
        if tracker {
            self.tracker_epoch = self.tracker_epoch.wrapping_add(1);
            self.issues = None;
            self.issue_options = OptionsState::NotAsked;
        }
    }

    pub fn known_me(&self) -> Option<&Me> {
        match &self.me {
            MeState::Known(m) => Some(m),
            _ => None,
        }
    }

    pub fn options(&self, kind: FilterKind) -> &OptionsState {
        match kind {
            FilterKind::Prs => &self.pr_options,
            FilterKind::Issues => &self.issue_options,
        }
    }

    pub fn options_mut(&mut self, kind: FilterKind) -> &mut OptionsState {
        match kind {
            FilterKind::Prs => &mut self.pr_options,
            FilterKind::Issues => &mut self.issue_options,
        }
    }
}

/// A line above a section's rows.
#[derive(Clone, Debug, PartialEq)]
pub enum Hint {
    Faint(String),
    /// A failed call: drawn in red with the sign-in buttons.
    Error(String),
}

pub struct PrView<'a> {
    pub rows: Vec<&'a PullRequest>,
    /// The shown filter's name.
    pub label: String,
    pub hint: Option<Hint>,
    /// What to say when there are no rows.
    pub empty: Option<String>,
}

/// The pull requests the PULL REQUESTS section shows: the loaded open ones under the chosen
/// filter's conditions (before the sidebar's text filter).
pub fn pr_view<'a>(fs: &'a ForgeState, f: &RepoForgeFilters) -> PrView<'a> {
    let (choice, c, label) = f.conditions(FilterKind::Prs);
    let (noun, total) = (fs.pr_noun(), fs.prs.len());
    let nothing_loaded = || if fs.loading > 0 { "Loading…".to_owned() } else { format!("No open {noun}s") };
    if choice == FilterChoice::default_for(FilterKind::Prs) {
        return PrView { rows: fs.prs.iter().collect(), label, hint: None, empty: (total == 0).then(nothing_loaded) };
    }
    if let Some(why) = fs.host_provider().and_then(|p| matching::unsupported(p, FilterKind::Prs, &c)) {
        return PrView { rows: Vec::new(), label, hint: Some(Hint::Faint(why)), empty: None };
    }
    let Some(rows) = matching::matching_prs(&fs.prs, &c, fs.filters.known_me()) else {
        let hint = match &fs.filters.me {
            MeState::Failed(e) => Hint::Error(e.clone()),
            _ => Hint::Faint(format!("Finding your {} account…", fs.host_provider().map(Provider::name).unwrap_or("hosting service"))),
        };
        return PrView { rows: Vec::new(), label, hint: Some(hint), empty: None };
    };
    let hint = Some(Hint::Faint(format!("{label} · {} of {total}", rows.len())));
    let empty = match (total, rows.len()) {
        (0, _) => Some(nothing_loaded()),
        (_, 0) => Some(format!("No open {noun}s match \u{201c}{label}\u{201d}")),
        _ => None,
    };
    PrView { rows, label, hint, empty }
}

pub struct IssueView<'a> {
    pub rows: &'a [Issue],
    pub label: String,
    pub hint: Hint,
}

/// The issues the ISSUES section shows: assigned ones from `my_issues` for the default filter,
/// otherwise the latest search for the chosen one. Nothing while the tracker failed, so its
/// error shows alone.
pub fn issue_view<'a>(fs: &'a ForgeState, f: &RepoForgeFilters) -> IssueView<'a> {
    let (choice, c, label) = f.conditions(FilterKind::Issues);
    let base = format!("{} · {label}", fs.tracker_hint());
    let view = |rows: &'a [Issue], hint| IssueView { rows, label: label.clone(), hint };
    if matches!(fs.tracker_status, Status::Error(_)) {
        return view(&[], Hint::Faint(base));
    }
    if choice == FilterChoice::default_for(FilterKind::Issues) {
        return view(&fs.my_issues, Hint::Faint(base));
    }
    if let Some(why) = fs.tracker.as_ref().and_then(|t| matching::unsupported(t.provider, FilterKind::Issues, &c)) {
        return view(&[], Hint::Faint(why));
    }
    match fs.filters.issues.as_ref().filter(|l| l.choice == choice && l.conditions == c) {
        Some(IssueList { status: Status::Ok, items, .. }) => view(items, Hint::Faint(base)),
        Some(IssueList { status: Status::Error(e), .. }) => view(&[], Hint::Error(e.clone())),
        Some(IssueList { items, .. }) => view(items, Hint::Faint(format!("{base} · Loading…"))),
        None => view(&[], Hint::Faint(format!("{base} · Loading…"))),
    }
}

/// "Labels: a, b · Milestone: m" for an issue's tooltip, when it has either.
pub fn facet_line(f: &IssueFacets) -> Option<String> {
    let mut parts = Vec::new();
    if !f.labels.is_empty() {
        parts.push(format!("Labels: {}", f.labels.join(", ")));
    }
    if let Some(m) = &f.milestone {
        parts.push(format!("Milestone: {m}"));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Where `kind`'s filters are applied: the code host for pull requests, the tracker for issues.
pub fn target_for(fs: &ForgeState, kind: FilterKind) -> Option<Target> {
    match kind {
        FilterKind::Prs => fs.host.clone(),
        FilterKind::Issues => fs.tracker.clone(),
    }
}

/// The options the filter dialog offers for `kind`: what the service sent, plus the labels and
/// milestones seen on the loaded pull requests or issues. None while they load.
pub fn suggestions(fs: &ForgeState, kind: FilterKind) -> Option<FilterOptions> {
    let mut out = match fs.filters.options(kind) {
        OptionsState::Loaded(o) => o.clone(),
        OptionsState::Failed(_) => FilterOptions::default(),
        OptionsState::NotAsked | OptionsState::Loading => return None,
    };
    let seen: Vec<(&[String], Option<&String>)> = match kind {
        FilterKind::Prs => fs.prs.iter().map(|p| (p.facets.labels.as_slice(), p.facets.milestone.as_ref())).collect(),
        FilterKind::Issues => {
            let listed = fs.filters.issues.iter().flat_map(|l| l.items.iter());
            fs.my_issues.iter().chain(listed).map(|i| (i.facets.labels.as_slice(), i.facets.milestone.as_ref())).collect()
        }
    };
    // A GitHub issue milestone is picked by number, which loaded issues don't carry.
    let numbered = kind == FilterKind::Issues && fs.tracker.as_ref().is_some_and(|t| matching::milestones_by_number(t.provider));
    for (labels, milestone) in seen {
        for l in labels {
            if !out.labels.iter().any(|have| have.eq_ignore_ascii_case(l)) {
                out.labels.push(l.clone());
            }
        }
        if let Some(m) = milestone.filter(|_| !numbered)
            && !out.milestones.iter().any(|have| have.title.eq_ignore_ascii_case(m))
        {
            out.milestones.push(Named { key: m.clone(), title: m.clone() });
        }
    }
    Some(out)
}
