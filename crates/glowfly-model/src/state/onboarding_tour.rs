//! The tour of the main window: its five steps, the panel each one points at, and their texts.
//! The tour's place lives in `Onboarding` (this session only, never saved).

use super::layout::Panel;
use crate::keymap::{BindingId, Keymap};

/// The tour's steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TourStep {
    Toolbar,
    LeftPanel,
    Graph,
    Details,
    Palette,
}

/// The card's text for a step whose panel is hidden.
pub const PANEL_HIDDEN: &str = "This panel is hidden.";
/// The card's text while the active tab has no repository.
pub const NO_REPOSITORY: &str = "Open a repository to see this part of the window.";

impl TourStep {
    pub const ALL: [TourStep; 5] = [TourStep::Toolbar, TourStep::LeftPanel, TourStep::Graph, TourStep::Details, TourStep::Palette];

    /// Its place, from 1.
    pub fn number(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0) + 1
    }

    pub fn next(self) -> Option<TourStep> {
        Self::ALL.get(self.number()).copied()
    }

    pub fn prev(self) -> Option<TourStep> {
        self.number().checked_sub(2).map(|i| Self::ALL[i])
    }

    /// The panel it points at, for the steps on a panel that can be hidden.
    pub fn panel(self) -> Option<Panel> {
        match self {
            TourStep::Toolbar => Some(Panel::Toolbar),
            TourStep::LeftPanel => Some(Panel::Left),
            TourStep::Details => Some(Panel::Details),
            TourStep::Graph | TourStep::Palette => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            TourStep::Toolbar => "The toolbar",
            TourStep::LeftPanel => "Branches and more",
            TourStep::Graph => "The commit graph",
            TourStep::Details => "Commit details",
            TourStep::Palette => "Commands and shortcuts",
        }
    }

    /// The card's sentences; the last step names the current keys from `keys`.
    pub fn body(self, keys: &Keymap) -> String {
        match self {
            TourStep::Toolbar => "The everyday actions live along the top: undo and redo, pull, push, a new branch, stash and pop, \
                                  pull requests and the terminal. At the right end are the command palette, search and Preferences."
                .into(),
            TourStep::LeftPanel => "Local and remote branches come first, then pull requests, issues, tags, stashes, submodules and \
                                    worktrees. Click a branch to find it in the graph, double-click to check it out, and right-click \
                                    any row for its actions."
                .into(),
            TourStep::Graph => "Every commit, with branches and tags drawn where they point. The WIP row at the top holds the \
                                changes you haven't committed yet. Click a commit to see what it changed, and right-click it for more."
                .into(),
            TourStep::Details => "The selected commit's message, author and files; click a file to see its diff. On the WIP row this \
                                  is where you stage changes, write a message and commit."
                .into(),
            TourStep::Palette => {
                let palette = match keys.hint(BindingId::Palette) {
                    Some(k) => format!("Press {k} to open the command palette"),
                    None => "Click Commands at the right of the toolbar to open the command palette".to_owned(),
                };
                let sheet = match keys.hint(BindingId::Sheet) {
                    Some(k) => format!("{k} lists every keyboard shortcut"),
                    None => "its Keyboard shortcuts entry lists every keyboard shortcut".to_owned(),
                };
                format!("{palette}: type a few letters of any command, branch or recent repository and press Enter. {sheet}.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn steps_in_order() {
        assert_eq!(TourStep::Toolbar.prev(), None);
        assert_eq!(TourStep::Toolbar.next(), Some(TourStep::LeftPanel));
        assert_eq!(TourStep::Palette.next(), None);
        assert_eq!(TourStep::Palette.prev(), Some(TourStep::Details));
        assert_eq!(TourStep::ALL.map(TourStep::number), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn the_last_step_names_the_current_keys() {
        let (keys, _) = Keymap::from_overrides(&BTreeMap::new());
        let body = TourStep::Palette.body(&keys);
        let (palette, sheet) = (keys.hint(BindingId::Palette).unwrap(), keys.hint(BindingId::Sheet).unwrap());
        assert!(body.contains(&format!("Press {palette}")) && body.contains(&format!("{sheet} lists")), "{body}");
        let unbound = BTreeMap::from([("palette".to_owned(), Vec::new()), ("shortcut-sheet".to_owned(), Vec::new())]);
        let (keys, _) = Keymap::from_overrides(&unbound);
        let body = TourStep::Palette.body(&keys);
        assert!(body.starts_with("Click Commands") && body.contains("Keyboard shortcuts entry"), "{body}");
    }
}
