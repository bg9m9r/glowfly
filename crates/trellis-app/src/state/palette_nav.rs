//! The palette's page and end keys: where PageUp, PageDown, Home and End move the selection in
//! a list of rows. They clamp at the ends, unlike Up and Down, which wrap. Pure.

/// A jump of the palette selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Jump {
    PageUp,
    PageDown,
    First,
    Last,
}

/// The rows one page holds: the list's height over a row's, rounded down.
pub const PAGE_ROWS: usize = 12;

/// The selection after `j` in a list of `len` rows, from `selected` (clamped to the list
/// first); 0 in an empty list.
pub fn jumped(selected: usize, len: usize, j: Jump) -> usize {
    let Some(last) = len.checked_sub(1) else { return 0 };
    let sel = selected.min(last);
    match j {
        Jump::PageUp => sel.saturating_sub(PAGE_ROWS),
        Jump::PageDown => (sel + PAGE_ROWS).min(last),
        Jump::First => 0,
        Jump::Last => last,
    }
}

#[cfg(test)]
mod tests {
    use super::Jump::*;
    use super::*;

    #[test]
    fn empty_and_single_lists() {
        for j in [PageUp, PageDown, First, Last] {
            assert_eq!(jumped(3, 0, j), 0, "{j:?}");
            assert_eq!(jumped(0, 1, j), 0, "{j:?}");
        }
    }

    #[test]
    fn pages_clamp_without_wrapping() {
        assert_eq!(jumped(0, 30, PageDown), 12);
        assert_eq!(jumped(12, 30, PageDown), 24);
        assert_eq!(jumped(24, 30, PageDown), 29);
        assert_eq!(jumped(29, 30, PageDown), 29, "no wrap at the end");
        assert_eq!(jumped(5, 30, PageUp), 0);
        assert_eq!(jumped(0, 30, PageUp), 0, "no wrap at the start");
        assert_eq!(jumped(20, 30, PageUp), 8);
        assert_eq!(jumped(50, 30, PageUp), 17, "a stale selection is clamped first");
        assert_eq!(jumped(7, 30, Last), 29);
        assert_eq!(jumped(7, 30, First), 0);
    }
}
