//! Moving within the file history's File view: find, go to line, the cursor line and where the
//! view scrolls next.

use super::blame_find::BlameFind;

/// The File view's find, Line field, cursor and scroll; the view edits a copy and sends it back
/// as `EditMsg::FileNav`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextNav {
    pub find: BlameFind,
    /// The Line field's text.
    pub goto: String,
    /// Focus the Find field on the next frame.
    pub focus_find: bool,
    /// The clicked or landed-on line (0-based).
    pub cursor: Option<usize>,
    /// The first line on screen and how many fit (written by the view each frame).
    pub top: usize,
    pub visible: usize,
    /// A line to centre on the next frame (taken by the view).
    pub scroll_to: Option<usize>,
    /// Scroll back to the first line on the next frame (a new version is shown).
    pub to_top: bool,
}

impl TextNav {
    /// Makes `line` the cursor line and centres it.
    pub fn land_on(&mut self, line: usize) {
        self.cursor = Some(line);
        self.scroll_to = Some(line);
    }

    /// Takes the view's edited copy, all but the find's matches and current match (update's).
    pub fn apply_view(&mut self, edited: TextNav) {
        let find = BlameFind { hits: std::mem::take(&mut self.find.hits), current: self.find.current, query: edited.find.query };
        *self = TextNav { find, ..edited };
    }

    /// A new version is shown: back to its top with no cursor or matches; the find and Line
    /// fields keep their text.
    pub fn reset(&mut self) {
        self.find.hits.clear();
        self.find.current = None;
        self.cursor = None;
        self.scroll_to = None;
        self.top = 0;
        self.to_top = true;
    }
}
