//! Moving within the file history's File view: find, go to line, the cursor line and where the
//! view scrolls next.

use super::blame_find::BlameFind;

#[derive(Clone, Debug, Default)]
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
