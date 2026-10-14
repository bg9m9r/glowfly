//! Checking the Git Flow settings form: branch names and prefixes against git's naming rules and
//! against each other.

use super::{FlowKind, GitFlowSettings};
use crate::branch_ops::is_valid_branch_name;

/// Why the form's values can't be saved.
#[derive(Debug, PartialEq)]
pub enum SettingsProblem {
    EmptyBranch {
        development: bool,
    },
    InvalidBranch(String),
    SameBranches,
    /// One branch name is a folder of the other.
    BranchesClash,
    EmptyPrefix(FlowKind),
    InvalidPrefix(FlowKind),
    /// The two prefixes are the same, or one starts with the other.
    SamePrefixes(FlowKind, FlowKind),
    BranchHasPrefix {
        branch: String,
        prefix: String,
    },
    InvalidTagPrefix,
}

impl SettingsProblem {
    pub fn message(&self) -> String {
        match self {
            SettingsProblem::EmptyBranch { development: false } => "Enter a production branch name.".into(),
            SettingsProblem::EmptyBranch { development: true } => "Enter a development branch name.".into(),
            SettingsProblem::InvalidBranch(n) => format!("{n} isn't a valid branch name."),
            SettingsProblem::SameBranches => "The production and development branches must be different.".into(),
            SettingsProblem::BranchesClash => "The production and development branch names clash: one is a folder of the other.".into(),
            SettingsProblem::EmptyPrefix(k) => format!("Enter a {} prefix.", k.noun()),
            SettingsProblem::InvalidPrefix(k) => format!("The {} prefix can't start a branch name.", k.noun()),
            SettingsProblem::SamePrefixes(a, b) => {
                format!("The {} and {} prefixes must differ, and neither may start with the other.", a.noun(), b.noun())
            }
            SettingsProblem::BranchHasPrefix { branch, prefix } => format!("The branch {branch} can't start with the prefix {prefix}."),
            SettingsProblem::InvalidTagPrefix => "The version tag prefix can't start a tag name.".into(),
        }
    }
}

/// Checks the (trimmed) values against each other and git's naming rules.
pub fn check_settings(s: &GitFlowSettings) -> Result<(), SettingsProblem> {
    use SettingsProblem as P;
    for (name, development) in [(&s.production, false), (&s.development, true)] {
        if name.is_empty() {
            return Err(P::EmptyBranch { development });
        }
        if !is_valid_branch_name(name) {
            return Err(P::InvalidBranch(name.clone()));
        }
    }
    let (p, d) = (s.production.to_lowercase(), s.development.to_lowercase());
    if p == d {
        return Err(P::SameBranches);
    }
    if p.starts_with(&format!("{d}/")) || d.starts_with(&format!("{p}/")) {
        return Err(P::BranchesClash);
    }
    for k in FlowKind::ALL {
        let v = s.prefix(k);
        if v.is_empty() {
            return Err(P::EmptyPrefix(k));
        }
        if !is_valid_branch_name(&format!("{v}x")) {
            return Err(P::InvalidPrefix(k));
        }
    }
    for (i, a) in FlowKind::ALL.into_iter().enumerate() {
        for b in FlowKind::ALL.into_iter().skip(i + 1) {
            let (la, lb) = (s.prefix(a).to_lowercase(), s.prefix(b).to_lowercase());
            if la.starts_with(&lb) || lb.starts_with(&la) {
                return Err(P::SamePrefixes(a, b));
            }
        }
    }
    for branch in [&s.production, &s.development] {
        for k in FlowKind::ALL {
            let prefix = s.prefix(k);
            if branch.to_lowercase().starts_with(&prefix.to_lowercase()) {
                return Err(P::BranchHasPrefix { branch: branch.clone(), prefix: prefix.to_owned() });
            }
        }
    }
    if !s.version_tag.is_empty() && !git2::Reference::is_valid_name(&format!("refs/tags/{}1.0.0", s.version_tag)) {
        return Err(P::InvalidTagPrefix);
    }
    Ok(())
}
