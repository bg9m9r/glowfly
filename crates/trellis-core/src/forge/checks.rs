//! CI checks on a pull request's head commit, in a form common to every service.

/// The overall build result of a pull request's head commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    Passed,
    Failed,
    Pending,
}

/// CI checks on a pull request's head commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checks {
    pub state: CheckState,
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    /// False when not every check could be read (too many, or of a kind not understood);
    /// the counts then cover only those read and are not shown.
    pub complete: bool,
}

impl Checks {
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.pending
    }

    /// Overall state from counts: any failed check fails the run, else any pending one keeps it
    /// pending, else it passed.
    pub fn from_counts(passed: usize, failed: usize, pending: usize, complete: bool) -> Checks {
        let state = if failed > 0 {
            CheckState::Failed
        } else if pending > 0 {
            CheckState::Pending
        } else {
            CheckState::Passed
        };
        Checks { state, passed, failed, pending, complete }
    }

    /// One tooltip line, e.g. "Checks: 3 of 5 passed, 2 failed". Counts are shown only when
    /// every check was read; otherwise the overall state alone.
    pub fn summary(&self) -> String {
        if !self.complete || self.total() == 0 {
            let word = match self.state {
                CheckState::Passed => "passed",
                CheckState::Failed => "some failed",
                CheckState::Pending => "pending",
            };
            return format!("Checks: {word}");
        }
        let mut s = format!("Checks: {} of {} passed", self.passed, self.total());
        if self.failed > 0 {
            s.push_str(&format!(", {} failed", self.failed));
        }
        if self.pending > 0 {
            s.push_str(&format!(", {} pending", self.pending));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_use_counts_only_when_complete() {
        let cases = [
            ((6, 0, 0), CheckState::Passed, "Checks: 6 of 6 passed"),
            ((3, 2, 0), CheckState::Failed, "Checks: 3 of 5 passed, 2 failed"),
            ((4, 0, 2), CheckState::Pending, "Checks: 4 of 6 passed, 2 pending"),
            ((4, 1, 2), CheckState::Failed, "Checks: 4 of 7 passed, 1 failed, 2 pending"),
        ];
        for ((p, f, n), state, text) in cases {
            let c = Checks::from_counts(p, f, n, true);
            assert_eq!((c.state, c.summary().as_str(), c.total()), (state, text, p + f + n));
        }
        assert_eq!(Checks::from_counts(6, 0, 0, false).summary(), "Checks: passed");
        assert_eq!(Checks::from_counts(3, 2, 0, false).summary(), "Checks: some failed");
        assert_eq!(Checks::from_counts(4, 0, 2, false).summary(), "Checks: pending");
        // No contexts counted, but the service still gave a state.
        let none = |state| Checks { state, passed: 0, failed: 0, pending: 0, complete: true };
        assert_eq!(none(CheckState::Passed).summary(), "Checks: passed");
        assert_eq!(none(CheckState::Failed).summary(), "Checks: some failed");
        assert_eq!(none(CheckState::Pending).summary(), "Checks: pending");
        assert_eq!(Checks::from_counts(0, 0, 0, true).state, CheckState::Passed);
    }
}
