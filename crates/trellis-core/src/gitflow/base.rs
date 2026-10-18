//! The roles of the configured production and development branches, for warning before one of
//! them is deleted. Pure.

use super::{GitFlowConfig, GitFlowSettings};

/// Which configured base branch a branch is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowBase {
    Production,
    Development,
}

impl FlowBase {
    /// "production" or "development".
    pub fn noun(self) -> &'static str {
        match self {
            FlowBase::Production => "production",
            FlowBase::Development => "development",
        }
    }
}

impl GitFlowSettings {
    /// The role of `branch`, by exact (case-sensitive) name.
    pub fn base_role(&self, branch: &str) -> Option<FlowBase> {
        if branch == self.production {
            Some(FlowBase::Production)
        } else if branch == self.development {
            Some(FlowBase::Development)
        } else {
            None
        }
    }
}

impl GitFlowConfig {
    /// The role of `branch` when Git Flow is set up.
    pub fn base_role(&self, branch: &str) -> Option<FlowBase> {
        self.settings()?.base_role(branch)
    }
}
