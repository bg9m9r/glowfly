//! The Add repositories picker of a repository group (`Modal::AddToGroup`, ui/repo_group_picker.rs):
//! what it lists, taken when it opens (the favorites not in the group, ungrouped ones first,
//! then those of the other groups in group order; then the Recent rows not found missing), its
//! filter and the ticked rows, kept by path so a row the filter hides stays ticked.

use super::AppState;
use super::repo_groups::{members, ungrouped};
use super::repo_lists::{is_missing, matches_filter, recent_rows};
use std::path::PathBuf;

/// More rows than this and the picker has a filter field.
pub const FILTER_FROM: usize = 8;

/// One repository the picker offers.
#[derive(Clone, Debug, PartialEq)]
pub struct PickRow {
    pub path: PathBuf,
    /// The other group a favorite is in now (ticking it moves it).
    pub other_group: Option<usize>,
    /// A Recent row, which adding stars.
    pub recent: bool,
}

/// The Add repositories picker for group `group`.
#[derive(Clone, Debug, PartialEq)]
pub struct AddPicker {
    pub group: usize,
    pub rows: Vec<PickRow>,
    pub filter: String,
    pub ticked: Vec<PathBuf>,
}

impl AddPicker {
    /// The picker for group `g`, listing what can be added to it now.
    pub fn new(st: &AppState, g: usize) -> Self {
        let s = &st.settings;
        let fav = |path: PathBuf, other_group| PickRow { path, other_group, recent: false };
        let mut rows: Vec<PickRow> = ungrouped(s).into_iter().map(|p| fav(p, None)).collect();
        for o in (0..s.repo_groups.len()).filter(|&o| o != g) {
            rows.extend(members(s, o).into_iter().map(|p| fav(p, Some(o))));
        }
        let recent = recent_rows(st).into_iter().filter(|p| !is_missing(st, p));
        rows.extend(recent.map(|p| PickRow { path: p.clone(), other_group: None, recent: true }));
        Self { group: g, rows, filter: String::new(), ticked: Vec::new() }
    }

    /// Whether the list is long enough to have a filter field.
    pub fn filtered(&self) -> bool {
        self.rows.len() > FILTER_FROM
    }

    /// The rows the filter keeps.
    pub fn shown(&self) -> impl Iterator<Item = &PickRow> {
        let filter = if self.filtered() { self.filter.as_str() } else { "" };
        self.rows.iter().filter(move |r| matches_filter(&r.path, filter))
    }

    pub fn is_ticked(&self, r: &PickRow) -> bool {
        self.ticked.contains(&r.path)
    }

    /// Ticks or unticks row `r`.
    pub fn toggle(&mut self, r: &PickRow) {
        match self.ticked.iter().position(|p| *p == r.path) {
            Some(i) => {
                self.ticked.remove(i);
            }
            None => self.ticked.push(r.path.clone()),
        }
    }

    /// The ticked repositories in list order (not the order they were ticked), hidden ones too.
    pub fn picked(&self) -> Vec<PathBuf> {
        self.rows.iter().filter(|r| self.is_ticked(r)).map(|r| r.path.clone()).collect()
    }
}
