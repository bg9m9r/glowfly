//! How many commits a load reads at least, whatever `initial_commits` says.

use super::Settings;

pub const MIN_INITIAL_COMMITS: usize = 500;

impl Settings {
    pub fn commit_limit(&self) -> usize {
        self.initial_commits.max(MIN_INITIAL_COMMITS)
    }
}
