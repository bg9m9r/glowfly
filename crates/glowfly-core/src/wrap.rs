//! Breaking long diff lines into visual lines for the wrapped diff viewer. Like `tabs`, each
//! char counts as one column, and the text is broken after tabs have been expanded, so the tab
//! size moves the breaks. Pure: no git, no UI.

use crate::tabs;
use std::ops::Range;

/// The byte ranges of the visual lines of `text` (already tab-expanded) at most `cols` chars
/// wide (0 acts as 1). A line breaks after the last space that fits, unless that space starts
/// the visual line; otherwise it breaks mid-word at `cols`. The ranges are contiguous, cover
/// the whole text and sit on char boundaries; an empty text is one empty visual line.
pub fn segments(text: &str, cols: usize) -> impl Iterator<Item = Range<usize>> + '_ {
    Segments { text, cols: cols.max(1), pos: 0, done: false }
}

/// How many visual lines raw diff line `raw` takes with tab stops every `tab` columns, at
/// `cols` columns. Borrows when the line has no tabs, so it doesn't allocate.
pub fn line_count(raw: &str, tab: usize, cols: usize) -> usize {
    if raw.len() <= cols.max(1) && !raw.contains('\t') {
        return 1;
    }
    let e = tabs::expand(raw, tab);
    segments(&e.text, cols).count()
}

struct Segments<'a> {
    text: &'a str,
    cols: usize,
    pos: usize,
    done: bool,
}

impl Iterator for Segments<'_> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Range<usize>> {
        if self.done {
            return None;
        }
        let start = self.pos;
        // Byte just after the last space past the visual line's first char.
        let mut after_space = None;
        for (count, (i, c)) in self.text[start..].char_indices().enumerate() {
            if count == self.cols {
                let end = after_space.unwrap_or(start + i);
                self.pos = end;
                return Some(start..end);
            }
            if c == ' ' && i > 0 {
                after_space = Some(start + i + 1);
            }
        }
        self.done = true;
        Some(start..self.text.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(text: &str, cols: usize) -> Vec<&str> {
        segments(text, cols).map(|r| &text[r]).collect()
    }

    #[test]
    fn short_and_empty_lines_are_one_segment() {
        assert_eq!(segments("abc", 10).collect::<Vec<_>>(), vec![0..3]);
        assert_eq!(segments("", 10).collect::<Vec<_>>(), vec![0..0]);
        assert_eq!(parts("abcd", 4), ["abcd"], "exactly cols wide");
    }

    #[test]
    fn breaks_after_the_last_space() {
        assert_eq!(parts("let value = compute(x)", 12), ["let value = ", "compute(x)"]);
        assert_eq!(parts("aa bb cc", 4), ["aa ", "bb ", "cc"]);
    }

    #[test]
    fn long_words_break_hard() {
        assert_eq!(parts("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(parts("a bcdefghij", 4), ["a ", "bcde", "fghi", "j"]);
    }

    #[test]
    fn leading_spaces_dont_make_empty_segments() {
        assert_eq!(parts("    abcdefgh", 4), ["    ", "abcd", "efgh"]);
        assert_eq!(parts(" abcdefg", 4), [" abc", "defg"], "a space that starts the line is no break");
        assert!(segments("      x", 2).all(|r| !r.is_empty()));
    }

    #[test]
    fn multibyte_chars_stay_whole() {
        assert_eq!(parts("éééé", 3), ["ééé", "é"]);
        assert_eq!(parts("日本語テキスト", 2), ["日本", "語テ", "キス", "ト"]);
    }

    #[test]
    fn zero_cols_acts_as_one() {
        assert_eq!(parts("abc", 0), ["a", "b", "c"]);
    }

    #[test]
    fn segments_cover_the_text() {
        let texts = ["", "x", "fn main() { println!(\"hello, world\"); }", "  indented  with   gaps  ", "ééé ééé é", "a-b-c d"];
        for text in texts {
            for cols in 1..12 {
                let segs: Vec<_> = segments(text, cols).collect();
                assert_eq!(segs.first().map(|r| r.start), Some(0), "{text:?} at {cols}");
                assert_eq!(segs.last().map(|r| r.end), Some(text.len()), "{text:?} at {cols}");
                for w in segs.windows(2) {
                    assert_eq!(w[0].end, w[1].start, "{text:?} at {cols}");
                }
                for r in &segs {
                    let s = text.get(r.clone()).expect("on char boundaries");
                    assert!(s.chars().count() <= cols, "{s:?} in {text:?} at {cols}");
                    assert!(!s.is_empty() || text.is_empty(), "{text:?} at {cols}");
                }
            }
        }
    }

    #[test]
    fn line_count_expands_tabs_first() {
        assert_eq!(line_count("\tab", 4, 4), 2);
        assert_eq!(line_count("\tab", 4, 4), segments(&tabs::expand("\tab", 4).text, 4).count());
        assert_eq!(line_count("\tab", 2, 4), 1, "a smaller tab size fits");
        assert_eq!(line_count("", 4, 4), 1);
        assert_eq!(line_count("abcdefghij", 4, 4), 3);
    }
}
