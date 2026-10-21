//! The rebase panel's exec and break steps, its row selection, and the squash groups a typed
//! combined message belongs to.

use super::rebase::{Editing, RebasePanel};
use git2::Oid;
use glowfly_core::rebase::{self, TodoStep};

/// A step the panel can add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewStep {
    Exec,
    Break,
}

/// Each group's head and the commits that meld into it, in order.
pub(super) fn groups(items: &[TodoStep]) -> Vec<(Oid, Vec<Oid>)> {
    let mut out = Vec::new();
    for (i, step) in items.iter().enumerate() {
        let (Some(head), Some(end)) = (step.oid(), rebase::group_end(items, i)) else { continue };
        let members = items[i + 1..=end].iter().filter_map(TodoStep::commit).filter(|c| c.action.combines()).map(|c| c.oid).collect();
        out.push((head, members));
    }
    out
}

impl RebasePanel {
    /// Whether row `i` heads a squash group.
    pub fn heads_group(&self, i: usize) -> bool {
        rebase::group_end(&self.items, i).is_some()
    }

    /// Where a step added after row `after` goes: after it, or after its group's last row when
    /// it is in a squash group (a step can't split one); at the end without a row.
    pub fn insert_at(&self, after: Option<usize>) -> usize {
        let Some(i) = after.filter(|&i| i < self.items.len()) else { return self.items.len() };
        let group = (0..=i).rev().find_map(|h| rebase::group_end(&self.items, h).filter(|&end| end >= i));
        group.unwrap_or(i) + 1
    }

    /// Adds an exec or break step after row `after` and selects it; an exec step opens its
    /// command editor.
    pub fn add_step(&mut self, after: Option<usize>, step: NewStep) {
        let mut after = after;
        // An open editor closes first; an empty exec step it removes shifts the rows after it.
        if let Some(Editing::Command(k)) = self.editing
            && matches!(self.items.get(k), Some(TodoStep::Exec(c)) if c.trim().is_empty())
        {
            after = match after {
                Some(a) if a == k => k.checked_sub(1),
                Some(a) if a > k => Some(a - 1),
                a => a,
            };
        }
        self.close_editor();
        let before = groups(&self.items);
        let at = self.insert_at(after);
        self.items.insert(at, if step == NewStep::Exec { TodoStep::Exec(String::new()) } else { TodoStep::Break });
        self.selected = Some(at);
        self.prune_combined(&before);
        if step == NewStep::Exec {
            self.open_editor(at);
        }
    }

    /// Removes row `i` when it is an exec or break step.
    pub fn remove_step(&mut self, i: usize) {
        if !matches!(self.items.get(i), Some(TodoStep::Exec(_) | TodoStep::Break)) {
            return;
        }
        let before = groups(&self.items);
        self.items.remove(i);
        let n = self.items.len();
        self.selected = match self.selected {
            Some(s) if s == i => (n > 0).then(|| i.min(n - 1)),
            Some(s) if s > i => Some(s - 1),
            s => s,
        };
        match self.editing {
            Some(e) if e.row() == i => {
                self.editing = None;
                self.draft.clear();
            }
            Some(e) if e.row() > i => self.editing = Some(e.with_row(e.row() - 1)),
            _ => {}
        }
        self.prune_combined(&before);
    }

    /// The row a move of the selection by `delta` rows selects (the arrow keys), stopping at the
    /// ends; with none, Down selects the first row and Up the last. `None` for an empty list.
    pub fn next_selection(&self, delta: isize) -> Option<usize> {
        let n = self.items.len();
        if n == 0 {
            return None;
        }
        Some(match self.selected.filter(|&s| s < n) {
            None if delta > 0 => 0,
            None => n - 1,
            Some(s) => s.saturating_add_signed(delta).min(n - 1),
        })
    }

    /// Forgets a typed combined message whose group changed since `before` (a member moved
    /// in or out, changed its action, or a step split the group), or whose row no longer heads
    /// one; a combined editor on such a row closes.
    pub(super) fn prune_combined(&mut self, before: &[(Oid, Vec<Oid>)]) {
        let after = groups(&self.items);
        for item in self.items.iter_mut().filter_map(TodoStep::commit_mut) {
            if item.combined_message.is_none() {
                continue;
            }
            let was = before.iter().find(|(h, _)| *h == item.oid).map(|(_, m)| m);
            let now = after.iter().find(|(h, _)| *h == item.oid).map(|(_, m)| m);
            if now.is_none() || was != now {
                item.combined_message = None;
            }
        }
        if let Some(Editing::Combined(h)) = self.editing
            && !self.heads_group(h)
        {
            self.editing = None;
            self.draft.clear();
        }
    }
}

#[cfg(test)]
#[path = "rebase_steps_tests.rs"]
mod tests;
