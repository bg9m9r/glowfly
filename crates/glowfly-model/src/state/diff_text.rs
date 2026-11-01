//! Text selected in the open diff's code, as in an editor: from an anchor to a head, each a
//! character of a shown line, grown to whole words or lines after a double- or triple-click.
//! In split view it stays on the side the press landed on. The view writes it directly while
//! the pointer drags, as the terminal's selection is written; update clears it when another
//! diff, or another load of it, is shown and when the layout switches. Copying is `copy`, a
//! pure function of the lines shown: no line numbers or signs, tabs kept, `\n` between lines.

use glowfly_core::repo::DiffTarget;

/// A line of the diff as the view shows it: line `line` of hunk `hunk`, an unchanged line
/// filled in between hunks in the hunk view (`line` of the gap above hunk `hunk`), or line
/// `line` of the whole-file view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineRef {
    Hunk { hunk: usize, line: usize },
    Gap { hunk: usize, line: usize },
    Whole { line: usize },
}

/// A place in a line's text: before character `col` (a character index into the line as
/// loaded, tabs counting as one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextPos {
    pub line: LineRef,
    pub col: usize,
}

/// What a drag grows the selection by: characters, or after a double- or triple-click whole
/// words or whole lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Unit {
    #[default]
    Char,
    Word,
    Line,
}

/// Which column the selection is in: the inline view's one, or a side of the split view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextSide {
    Inline,
    Old,
    New,
}

/// The selection: from `anchor` (where the press landed) to `head` (where the pointer is),
/// on `side`, in the diff `key` names (its target and `DiffSlot::shown_gen`).
#[derive(Clone, Debug, PartialEq)]
pub struct TextSel {
    pub anchor: TextPos,
    pub head: TextPos,
    pub unit: Unit,
    pub side: TextSide,
    pub key: (DiffTarget, u64),
}

/// One line the selection's side shows, in display order: its place, origin (`+`, `-` or a
/// space) and text as loaded.
#[derive(Clone, Copy, Debug)]
pub struct ShownLine<'a> {
    pub line: LineRef,
    pub origin: char,
    pub text: &'a str,
}

/// The selected stretch in display order: from `start` to `end`, with the order each one's
/// line has among the shown lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: TextPos,
    pub start_at: usize,
    pub end: TextPos,
    pub end_at: usize,
}

impl Span {
    /// Whether nothing is selected (both ends at one place).
    pub fn is_empty(&self) -> bool {
        self.start_at == self.end_at && self.start.col == self.end.col
    }
}

impl TextSel {
    /// A selection of nothing yet at `at`, growing by `unit`.
    pub fn at(at: TextPos, unit: Unit, side: TextSide, key: (DiffTarget, u64)) -> Self {
        Self { anchor: at, head: at, unit, side, key }
    }

    /// The selected stretch: its ends put in display order by `order` (a line's place among
    /// those shown, `None` when it isn't shown) and grown to whole words or lines from the
    /// lines' `text`. `None` when an end's line isn't shown.
    pub fn span<'a>(&self, order: impl Fn(LineRef) -> Option<usize>, text: impl Fn(LineRef) -> Option<&'a str>) -> Option<Span> {
        let (a, b) = (order(self.anchor.line)?, order(self.head.line)?);
        let ((start_at, start), (end_at, end)) = if (a, self.anchor.col) <= (b, self.head.col) {
            ((a, self.anchor), (b, self.head))
        } else {
            ((b, self.head), (a, self.anchor))
        };
        let len = |p: TextPos| text(p.line).map_or(0, |t| t.chars().count());
        let (start_col, end_col) = match self.unit {
            Unit::Char => (start.col.min(len(start)), end.col.min(len(end))),
            Unit::Word => {
                let (s, _) = text(start.line).map_or((0, 0), |t| word_at(t, start.col));
                let (_, e) = text(end.line).map_or((0, 0), |t| word_at(t, end.col));
                (s, e)
            }
            Unit::Line => (0, len(end)),
        };
        Some(Span { start: TextPos { col: start_col, ..start }, start_at, end: TextPos { col: end_col, ..end }, end_at })
    }
}

/// What a character is for word selection: part of a word (a letter, a digit or `_`),
/// whitespace, or punctuation, which makes a word only with copies of itself.
fn class(c: char) -> (u8, Option<char>) {
    if c.is_alphanumeric() || c == '_' {
        (0, None)
    } else if c.is_whitespace() {
        (1, None)
    } else {
        (2, Some(c))
    }
}

/// The word around character `col` of `text`, as character indexes `(start, end)`: a run of
/// letters, digits and `_`, a run of one repeated punctuation character, or a run of
/// whitespace. At the end of the line, the word before it; an empty line gives `(0, 0)`.
pub fn word_at(text: &str, col: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let at = col.min(chars.len() - 1);
    let kind = class(chars[at]);
    let mut start = at;
    while start > 0 && class(chars[start - 1]) == kind {
        start -= 1;
    }
    let mut end = at + 1;
    while end < chars.len() && class(chars[end]) == kind {
        end += 1;
    }
    (start, end)
}

/// The text `sel` selects among `shown`, the lines of its side in display order: each line's
/// selected characters as loaded (tabs kept), joined by `\n`, with nothing for wrapping, folds
/// or hunk headers. With `signs`, each line starts with its origin (`+`, `-` or a space).
/// Empty when an end isn't shown or nothing is selected.
pub fn copy(shown: &[ShownLine], sel: &TextSel, signs: bool) -> String {
    let order = |r: LineRef| shown.iter().position(|l| l.line == r);
    let text = |r: LineRef| shown.iter().find(|l| l.line == r).map(|l| l.text);
    let Some(span) = sel.span(order, text) else { return String::new() };
    if span.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, l) in shown.iter().enumerate().take(span.end_at + 1).skip(span.start_at) {
        let from = if i == span.start_at { span.start.col } else { 0 };
        let to = if i == span.end_at { span.end.col } else { usize::MAX };
        if signs {
            out.push(if matches!(l.origin, '+' | '-') { l.origin } else { ' ' });
        }
        out.extend(l.text.chars().skip(from).take(to.saturating_sub(from)));
        if i < span.end_at {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
#[path = "diff_text_tests.rs"]
mod tests;
