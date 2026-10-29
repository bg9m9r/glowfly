//! Which toolbar parts fold into the ⋯ More menu in a narrow window. The pickers first narrow
//! to a comfortable width (`toolbar_pickers::COMFORT`), then the parts fold in `ORDER`: the
//! Terminal and Folder group, Undo and Redo, Branch, Stash and Pop, the pull request group,
//! then Commands and Settings out of the right-hand group. The pickers, Pull and Push, Search,
//! Activity, Refresh and the chips never fold; once everything that can has folded, the pickers
//! narrow on to their floors. The decision comes from the parts' full widths and the room, never
//! from what was drawn, so the bar can't flicker between two layouts.

/// A part of the toolbar that can fold.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Files,
    Undo,
    Branch,
    Pr,
    Commands,
    Settings,
}

/// The parts in the order they fold.
pub const ORDER: [Part; 6] = [Part::Files, Part::Undo, Part::Branch, Part::Pr, Part::Commands, Part::Settings];

/// The widths the decision needs: the bar's `room`, the pickers at their comfortable width,
/// what never folds (the gaps, Pull and Push, the right-hand group's three fixed buttons and
/// the chips), the More button's group with its gap, and each foldable part's width with its
/// gap or spacing, in `ORDER` (0 for a part not shown, such as the pull request group without
/// a hosting service).
pub struct Widths {
    pub room: f32,
    pub pickers: f32,
    pub fixed: f32,
    pub more: f32,
    pub parts: [f32; ORDER.len()],
}

/// How many of `ORDER` fold: the fewest that leave everything else room, or all of them.
pub fn folded(w: &Widths) -> usize {
    (0..=ORDER.len())
        .find(|&k| {
            let shown: f32 = w.parts[k..].iter().sum();
            let more = if w.parts[..k].iter().any(|&p| p > 0.0) { w.more } else { 0.0 };
            w.pickers + w.fixed + shown + more <= w.room
        })
        .unwrap_or(ORDER.len())
}

/// Whether `part` is folded when the first `k` of `ORDER` are.
pub fn is_folded(part: Part, k: usize) -> bool {
    ORDER[..k].contains(&part)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths(room: f32) -> Widths {
        Widths { room, pickers: 300.0, fixed: 400.0, more: 70.0, parts: [130.0, 130.0, 190.0, 90.0, 58.0, 58.0] }
    }

    #[test]
    fn nothing_folds_when_everything_fits() {
        let all = 300.0 + 400.0 + 130.0 + 130.0 + 190.0 + 90.0 + 58.0 + 58.0;
        assert_eq!(folded(&widths(all)), 0);
        assert_eq!(folded(&widths(f32::INFINITY)), 0);
    }

    #[test]
    fn parts_fold_in_order_and_the_more_button_counts() {
        let all = 300.0 + 400.0 + 130.0 + 130.0 + 190.0 + 90.0 + 58.0 + 58.0;
        // One pixel short: Files folds, but More takes 70 of the 130 it frees.
        assert_eq!(folded(&widths(all - 1.0)), 1);
        assert_eq!(folded(&widths(all - 60.0)), 1);
        assert_eq!(folded(&widths(all - 61.0)), 2);
        assert_eq!(folded(&widths(300.0 + 400.0 + 70.0)), 6);
        assert_eq!(folded(&widths(10.0)), 6, "a tiny room folds everything");
        assert!(is_folded(Part::Files, 1) && !is_folded(Part::Undo, 1));
    }

    #[test]
    fn a_part_not_shown_folds_for_free() {
        let mut w = widths(0.0);
        w.parts[3] = 0.0;
        // Everything but Settings folded: More, and the pull request group was never there.
        w.room = 300.0 + 400.0 + 70.0 + 58.0;
        assert_eq!(folded(&w), 5);
        // Only the missing group "folded": no More button.
        let w = Widths { room: 700.0, pickers: 300.0, fixed: 400.0, more: 70.0, parts: [0.0; 6] };
        assert_eq!(folded(&w), 0);
    }
}
