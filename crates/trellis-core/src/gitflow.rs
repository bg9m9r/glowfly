//! Git Flow config, defaults, checks and command lines for setting up, starting and finishing
//! (`finish.rs`) feature, release and hotfix branches, implemented natively with plain git, and
//! the local flow branches grouped by kind (`groups.rs`).
//! The settings live in the repository's git config under the `gitflow.*` keys the git-flow
//! command-line tool also uses, so either can work in the same repository. Only `read_config`
//! touches a repository; everything else is pure.

use crate::branch_ops::{NameProblem, check_branch_name};
use crate::gitcli::args;
use crate::repo::Snapshot;

mod checks;
mod finish;
#[cfg(test)]
mod finish_tests;
mod groups;
mod start;
#[cfg(test)]
mod tests;
mod version;

pub use checks::{SettingsProblem, check_settings};
pub use finish::{
    FinishOptions, FinishPlan, FinishProblem, FinishStep, WorkState, check_finish, default_tag, default_tag_message, flow_branches,
    flow_kind, short_name, targets,
};
pub use groups::{FlowBranch, FlowGroup, grouped, total};
pub use start::{StartProblem, check_start, open_branches, start_cmd};
pub use version::suggest_version;

/// The kinds of branch Git Flow starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowKind {
    Feature,
    Release,
    Hotfix,
}

impl FlowKind {
    pub const ALL: [FlowKind; 3] = [FlowKind::Feature, FlowKind::Release, FlowKind::Hotfix];

    /// "feature", "release" or "hotfix" (also the config key's last part).
    pub fn noun(self) -> &'static str {
        match self {
            FlowKind::Feature => "feature",
            FlowKind::Release => "release",
            FlowKind::Hotfix => "hotfix",
        }
    }

    /// Whether the branch is named after a version (and tagged when finished).
    pub fn versioned(self) -> bool {
        matches!(self, FlowKind::Release | FlowKind::Hotfix)
    }
}

pub const KEY_PRODUCTION: &str = "gitflow.branch.master";
pub const KEY_DEVELOPMENT: &str = "gitflow.branch.develop";

/// "gitflow.prefix.<name>".
pub fn prefix_key(name: &str) -> String {
    format!("gitflow.prefix.{name}")
}

pub const DEFAULT_DEVELOPMENT: &str = "develop";
pub const DEFAULT_FEATURE: &str = "feature/";
pub const DEFAULT_RELEASE: &str = "release/";
pub const DEFAULT_HOTFIX: &str = "hotfix/";
pub const DEFAULT_BUGFIX: &str = "bugfix/";
pub const DEFAULT_SUPPORT: &str = "support/";

/// What the config holds; None means the key is absent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitFlowConfig {
    pub production: Option<String>,
    pub development: Option<String>,
    pub feature: Option<String>,
    pub release: Option<String>,
    pub hotfix: Option<String>,
    pub version_tag: Option<String>,
    pub bugfix: Option<String>,
    pub support: Option<String>,
}

impl GitFlowConfig {
    /// Both branches are named and differ, and the three prefixes are configured.
    pub fn is_initialized(&self) -> bool {
        let named = |b: &Option<String>| b.as_deref().filter(|n| !n.is_empty()).map(str::to_lowercase);
        let branches = match (named(&self.production), named(&self.development)) {
            (Some(p), Some(d)) => p != d,
            _ => false,
        };
        branches && self.feature.is_some() && self.release.is_some() && self.hotfix.is_some()
    }

    /// The configured values, when the repository is initialized.
    pub fn settings(&self) -> Option<GitFlowSettings> {
        if !self.is_initialized() {
            return None;
        }
        let v = |o: &Option<String>| o.clone().unwrap_or_default();
        Some(GitFlowSettings {
            production: v(&self.production),
            development: v(&self.development),
            feature: v(&self.feature),
            release: v(&self.release),
            hotfix: v(&self.hotfix),
            version_tag: v(&self.version_tag),
        })
    }
}

/// Reads the `gitflow.*` values from the merged config (local, global and system). A key
/// without a value reads as "". Errors read as an empty config.
pub fn read_config(repo: &git2::Repository) -> GitFlowConfig {
    let read = || -> Result<GitFlowConfig, git2::Error> {
        let cfg = repo.config()?.snapshot()?;
        // `value()` panics on a key without a value, so check first.
        let get = |key: &str| {
            let e = cfg.get_entry(key).ok()?;
            Some(if e.has_value() { String::from_utf8_lossy(e.value_bytes()).into_owned() } else { String::new() })
        };
        Ok(GitFlowConfig {
            production: get(KEY_PRODUCTION),
            development: get(KEY_DEVELOPMENT),
            feature: get(&prefix_key("feature")),
            release: get(&prefix_key("release")),
            hotfix: get(&prefix_key("hotfix")),
            version_tag: get(&prefix_key("versiontag")),
            bugfix: get(&prefix_key("bugfix")),
            support: get(&prefix_key("support")),
        })
    };
    read().unwrap_or_default()
}

/// The editable values, all present.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitFlowSettings {
    pub production: String,
    pub development: String,
    pub feature: String,
    pub release: String,
    pub hotfix: String,
    /// Put in front of the version in tag names; may be empty.
    pub version_tag: String,
}

impl GitFlowSettings {
    pub fn prefix(&self, k: FlowKind) -> &str {
        match k {
            FlowKind::Feature => &self.feature,
            FlowKind::Release => &self.release,
            FlowKind::Hotfix => &self.hotfix,
        }
    }

    /// The branch a new branch of kind `k` starts from.
    pub fn base(&self, k: FlowKind) -> &str {
        match k {
            FlowKind::Feature | FlowKind::Release => &self.development,
            FlowKind::Hotfix => &self.production,
        }
    }

    /// Every value with surrounding whitespace removed.
    pub fn trimmed(&self) -> Self {
        let t = |s: &str| s.trim().to_owned();
        Self {
            production: t(&self.production),
            development: t(&self.development),
            feature: t(&self.feature),
            release: t(&self.release),
            hotfix: t(&self.hotfix),
            version_tag: t(&self.version_tag),
        }
    }
}

/// The refs the checks run against.
pub struct Branches<'a> {
    pub locals: Vec<&'a str>,
    /// (remote, branch): origin's first, then in snapshot order; "HEAD" excluded.
    pub remote: Vec<(&'a str, &'a str)>,
    pub tags: Vec<&'a str>,
    /// The checked-out branch.
    pub head: Option<&'a str>,
    pub has_commits: bool,
}

impl<'a> Branches<'a> {
    pub fn of(snap: &'a Snapshot) -> Self {
        let locals: Vec<&str> = snap.locals.iter().map(|b| b.name.as_str()).collect();
        let mut remotes: Vec<_> = snap.remotes.iter().collect();
        remotes.sort_by_key(|(r, _)| r != "origin");
        let remote = remotes
            .into_iter()
            .flat_map(|(r, bs)| bs.iter().filter(|(b, _)| b != "HEAD").map(move |(b, _)| (r.as_str(), b.as_str())))
            .collect();
        Branches {
            has_commits: snap.head_oid.is_some() || !locals.is_empty(),
            locals,
            remote,
            tags: snap.tags.iter().map(|(t, _)| t.as_str()).chain(snap.other_tags.iter().map(String::as_str)).collect(),
            head: snap.head_branch.as_deref(),
        }
    }

    pub fn local(&self, n: &str) -> bool {
        self.locals.contains(&n)
    }

    /// "<remote>/<n>" for the first remote that has `n`.
    pub fn on_remote(&self, n: &str) -> Option<String> {
        self.remote.iter().find(|(_, b)| *b == n).map(|(r, b)| format!("{r}/{b}"))
    }
}

/// The form's starting values: each configured value, else its default.
pub fn suggested(cfg: &GitFlowConfig, b: &Branches) -> GitFlowSettings {
    let or = |o: &Option<String>, d: &str| o.clone().unwrap_or_else(|| d.to_owned());
    GitFlowSettings {
        production: cfg.production.clone().unwrap_or_else(|| default_production(b)),
        development: or(&cfg.development, DEFAULT_DEVELOPMENT),
        feature: or(&cfg.feature, DEFAULT_FEATURE),
        release: or(&cfg.release, DEFAULT_RELEASE),
        hotfix: or(&cfg.hotfix, DEFAULT_HOTFIX),
        version_tag: or(&cfg.version_tag, ""),
    }
}

/// main or master locally, else on a remote, else the checked-out branch, else main.
fn default_production(b: &Branches) -> String {
    let names = ["main", "master"];
    let found = names.iter().find(|n| b.local(n)).or_else(|| names.iter().find(|n| b.on_remote(n).is_some()));
    found.copied().or(b.head).unwrap_or("main").to_owned()
}

/// How Initialize or Save gets a configured branch.
#[derive(Debug, PartialEq)]
pub enum BranchPlan {
    Exists,
    /// Created tracking this remote branch ("origin/develop").
    FromRemote(String),
    /// Created at the production branch (development only).
    FromProduction,
}

/// Why Initialize or Save can't go ahead with these branches.
#[derive(Debug, PartialEq)]
pub enum InitProblem {
    NoCommits,
    MissingProduction(String),
    /// The development branch must be created, and its name is taken or clashes.
    DevelopmentName(NameProblem, String),
}

impl InitProblem {
    pub fn message(&self) -> String {
        match self {
            InitProblem::NoCommits => "Make a first commit before setting up Git Flow.".into(),
            InitProblem::MissingProduction(n) => format!("{n} doesn't exist."),
            InitProblem::DevelopmentName(p, n) => p.message(n),
        }
    }
}

/// The plans for (production, development). Production is never created from nothing.
pub fn plan_branches(s: &GitFlowSettings, b: &Branches) -> Result<(BranchPlan, BranchPlan), InitProblem> {
    if !b.has_commits {
        return Err(InitProblem::NoCommits);
    }
    let production = if b.local(&s.production) {
        BranchPlan::Exists
    } else if let Some(r) = b.on_remote(&s.production) {
        BranchPlan::FromRemote(r)
    } else {
        return Err(InitProblem::MissingProduction(s.production.clone()));
    };
    let development = if b.local(&s.development) {
        BranchPlan::Exists
    } else {
        check_branch_name(&s.development, &b.locals, None).map_err(|p| InitProblem::DevelopmentName(p, s.development.clone()))?;
        b.on_remote(&s.development).map_or(BranchPlan::FromProduction, BranchPlan::FromRemote)
    };
    Ok((production, development))
}

/// Initialize or Save: create any missing branch, then write the config (to the repository's
/// own config). Branches come first so a failure leaves the repository uninitialized. The
/// bugfix and support prefixes are written only when absent, for the git-flow tool's sake.
pub fn init_cmds(s: &GitFlowSettings, cfg: &GitFlowConfig, b: &Branches) -> Result<Vec<Vec<String>>, InitProblem> {
    let (production, development) = plan_branches(s, b)?;
    let mut cmds = Vec::new();
    if let BranchPlan::FromRemote(r) = &production {
        cmds.push(args(&["branch", "--track", &s.production, r]));
    }
    match &development {
        BranchPlan::FromRemote(r) => cmds.push(args(&["branch", "--track", &s.development, r])),
        BranchPlan::FromProduction => cmds.push(args(&["branch", "--no-track", &s.development, &format!("refs/heads/{}", s.production)])),
        BranchPlan::Exists => {}
    }
    let mut set = |key: &str, value: &str| cmds.push(args(&["config", "--local", key, value]));
    set(KEY_PRODUCTION, &s.production);
    set(KEY_DEVELOPMENT, &s.development);
    set(&prefix_key("feature"), &s.feature);
    if cfg.bugfix.is_none() {
        set(&prefix_key("bugfix"), DEFAULT_BUGFIX);
    }
    set(&prefix_key("release"), &s.release);
    set(&prefix_key("hotfix"), &s.hotfix);
    if cfg.support.is_none() {
        set(&prefix_key("support"), DEFAULT_SUPPORT);
    }
    set(&prefix_key("versiontag"), &s.version_tag);
    Ok(cmds)
}
