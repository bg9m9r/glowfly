//! The keys of a file list that has the keyboard focus (`ui/list_focus.rs` reads them, `update`
//! acts on them) and where a move lands in the list's visible order. Shared by every list that
//! takes the focus as a whole; no egui types.

/// One key of a focused file list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListKey {
    /// Up/Down, Page Up/Page Down, Home/End: open the file it lands on.
    Move(ListMove),
    /// The same with Shift: open the file it lands on and select every file from the anchor.
    Extend(ListMove),
    /// Space: stage or unstage the file or the selected files.
    Toggle,
    /// Enter: open the list's current file.
    Open,
    /// Delete (or Mod+Backspace): discard the current file's changes, asking first.
    Delete,
    /// Left (`right: false`) or Right: go to the list on that side.
    Across { right: bool },
}

/// Where a move goes from the current file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListMove {
    /// The next (`down`) or previous file.
    Step { down: bool },
    /// `rows` files on or back (a page of the list), stopping at either end.
    Page { down: bool, rows: usize },
    /// The last (`end`) or first file.
    Edge { end: bool },
}

/// Where the current file sits in a list's visible order (`len` files).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    /// It isn't in the list.
    Outside,
    /// The visible file at this position.
    At(usize),
    /// Hidden (in a collapsed folder), with this many visible files before it.
    Hidden { before: usize },
}

impl ListMove {
    /// The visible position the move lands on in a list of `len` visible files, from `at`.
    /// None when there is nothing to move to: an empty list, a step past either end, or a page
    /// or edge move that stays on the file it starts from. From outside the list any move but
    /// Edge lands on the first file.
    pub fn target(self, len: usize, at: Place) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let last = len - 1;
        let to = match (self, at) {
            (ListMove::Edge { end }, _) => Some(if end { last } else { 0 }),
            (_, Place::Outside) => Some(0),
            (ListMove::Step { down: true }, Place::At(i)) => Some(i + 1).filter(|&j| j < len),
            (ListMove::Step { down: false }, Place::At(i)) => i.checked_sub(1),
            (ListMove::Step { down: true }, Place::Hidden { before }) => Some(before).filter(|&j| j < len),
            (ListMove::Step { down: false }, Place::Hidden { before }) => before.checked_sub(1),
            (ListMove::Page { down: true, rows }, Place::At(i)) => Some((i + rows.max(1)).min(last)),
            (ListMove::Page { down: false, rows }, Place::At(i)) => Some(i.saturating_sub(rows.max(1))),
            (ListMove::Page { down: true, rows }, Place::Hidden { before }) => Some((before + rows.max(1) - 1).min(last)),
            (ListMove::Page { down: false, rows }, Place::Hidden { before }) => Some(before.saturating_sub(rows.max(1))),
        };
        to.filter(|&j| at != Place::At(j))
    }
}

/// The place of the file at `index` (into the list's files) in `order`: the files in on-screen
/// order, each with whether it is shown.
pub fn place_in(order: &[(usize, bool)], index: Option<usize>) -> Place {
    let Some(index) = index else { return Place::Outside };
    let mut before = 0;
    for &(i, shown) in order {
        if i == index {
            return if shown { Place::At(before) } else { Place::Hidden { before } };
        }
        before += usize::from(shown);
    }
    Place::Outside
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOWN: ListMove = ListMove::Step { down: true };
    const UP: ListMove = ListMove::Step { down: false };

    #[test]
    fn steps_stop_at_the_ends() {
        assert_eq!(DOWN.target(3, Place::At(0)), Some(1));
        assert_eq!(DOWN.target(3, Place::At(2)), None);
        assert_eq!(UP.target(3, Place::At(0)), None);
        assert_eq!(UP.target(3, Place::At(2)), Some(1));
        assert_eq!(DOWN.target(0, Place::Outside), None);
    }

    #[test]
    fn from_outside_every_move_but_edge_starts_at_the_first_file() {
        assert_eq!(UP.target(3, Place::Outside), Some(0));
        assert_eq!(ListMove::Page { down: true, rows: 5 }.target(3, Place::Outside), Some(0));
        assert_eq!(ListMove::Edge { end: true }.target(3, Place::Outside), Some(2));
    }

    #[test]
    fn pages_clamp_to_the_ends_and_stay_put_at_them() {
        let pg = |down, rows| ListMove::Page { down, rows };
        assert_eq!(pg(true, 4).target(10, Place::At(1)), Some(5));
        assert_eq!(pg(true, 4).target(10, Place::At(8)), Some(9));
        assert_eq!(pg(true, 4).target(10, Place::At(9)), None);
        assert_eq!(pg(false, 4).target(10, Place::At(2)), Some(0));
        assert_eq!(pg(false, 4).target(10, Place::At(0)), None);
        assert_eq!(pg(true, 0).target(10, Place::At(0)), Some(1), "a page is at least one row");
    }

    #[test]
    fn edges_go_to_the_first_or_last_file() {
        assert_eq!(ListMove::Edge { end: false }.target(4, Place::At(2)), Some(0));
        assert_eq!(ListMove::Edge { end: true }.target(4, Place::At(2)), Some(3));
        assert_eq!(ListMove::Edge { end: true }.target(4, Place::At(3)), None);
    }

    #[test]
    fn a_hidden_file_moves_to_its_visible_neighbours() {
        // Visible files 0 and 1 before it, 2 after.
        let at = Place::Hidden { before: 2 };
        assert_eq!(DOWN.target(3, at), Some(2));
        assert_eq!(UP.target(3, at), Some(1));
        assert_eq!(DOWN.target(2, at), None);
        assert_eq!(UP.target(3, Place::Hidden { before: 0 }), None);
        assert_eq!(ListMove::Page { down: true, rows: 2 }.target(5, at), Some(3));
    }

    #[test]
    fn places_count_only_shown_files() {
        let order = [(2, true), (0, false), (1, true)];
        assert_eq!(place_in(&order, Some(2)), Place::At(0));
        assert_eq!(place_in(&order, Some(0)), Place::Hidden { before: 1 });
        assert_eq!(place_in(&order, Some(1)), Place::At(1));
        assert_eq!(place_in(&order, Some(7)), Place::Outside);
        assert_eq!(place_in(&order, None), Place::Outside);
    }
}
