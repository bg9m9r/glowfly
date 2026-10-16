//! The small queries on a loaded `Snapshot`.

use super::*;
use std::collections::HashSet;

impl Snapshot {
    /// Loaded commits reachable from `from` but not from `exclude` (newest first), e.g. the
    /// commits a branch would contribute to its target. Limited to the loaded history.
    pub fn commits_between(&self, from: Oid, exclude: Option<Oid>) -> Vec<&CommitInfo> {
        let reach = |start: Oid| {
            let mut seen = HashSet::new();
            let mut stack = vec![start];
            while let Some(o) = stack.pop() {
                if let Some(&row) = self.row_of.get(&o)
                    && seen.insert(o)
                {
                    stack.extend(self.commits[row].parents.iter().copied());
                }
            }
            seen
        };
        let excluded = exclude.map(reach).unwrap_or_default();
        let wanted = reach(from);
        self.commits.iter().filter(|c| wanted.contains(&c.oid) && !excluded.contains(&c.oid)).collect()
    }

    pub fn is_dirty(&self) -> bool {
        !self.unstaged.is_empty() || !self.staged.is_empty()
    }

    /// Why `core.ignorecase` and the file system disagree, when they do: git then matches
    /// paths differently from the disk (a repository copied between systems).
    pub fn case_mismatch(&self) -> Option<&'static str> {
        match (self.folding.case, self.ignorecase) {
            (Some(true), false) => Some("This file system ignores case, but core.ignorecase is false: git may see one file as two."),
            (Some(false), true) => Some("This file system is case-sensitive, but core.ignorecase is true: git may match the wrong file."),
            _ => None,
        }
    }

    pub fn head_info(&self) -> Option<&BranchInfo> {
        self.locals.iter().find(|b| b.is_head)
    }

    /// Whether status says `path` is gone from the working tree: deleted there (unstaged 'D'),
    /// or deleted in the index ('D' staged) with nothing at the path since.
    pub fn missing_from_workdir(&self, path: &str) -> bool {
        if let Some(u) = self.unstaged.iter().find(|f| f.path == path) {
            return u.status == 'D';
        }
        self.staged.iter().any(|f| f.path == path && f.status == 'D')
    }
}
