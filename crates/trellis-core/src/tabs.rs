//! Tab stops for drawing diff text. Loaded lines keep their raw tabs; the viewer expands them
//! when it paints a line, so the tab size can change without reloading. The byte-offset map
//! lets syntax spans and changed-word ranges (byte ranges into the raw text) follow the line
//! onto its expanded form. Pure: no git, no UI.

use std::borrow::Cow;

/// Display columns of `text` with a tab stop every `tab` columns (each char is one column; tab
/// 0 acts as 1).
pub fn columns(text: &str, tab: usize) -> usize {
    let tab = tab.max(1);
    text.chars().fold(0, |col, c| if c == '\t' { (col / tab + 1) * tab } else { col + 1 })
}

/// `text` with each tab replaced by spaces up to the next stop, plus a byte-offset map.
pub struct Expanded<'a> {
    pub text: Cow<'a, str>,
    /// For each byte of the original (and one past its end), the matching byte in `text`;
    /// `None` when there were no tabs, so offsets are unchanged.
    offsets: Option<Vec<u32>>,
}

impl Expanded<'_> {
    /// The offset in `text` matching byte `b` of the original. Identity when the line had no
    /// tabs; u32::MAX past the end, so bad ranges stay bad and are rejected by the painter.
    pub fn map(&self, b: u32) -> u32 {
        match &self.offsets {
            None => b,
            Some(o) => o.get(b as usize).copied().unwrap_or(u32::MAX),
        }
    }
}

/// Expands the tabs in `text` to stops every `tab` columns (tab 0 acts as 1). Without tabs the
/// text is borrowed as it is.
pub fn expand(text: &str, tab: usize) -> Expanded<'_> {
    if !text.contains('\t') {
        return Expanded { text: Cow::Borrowed(text), offsets: None };
    }
    let tab = tab.max(1);
    let mut out = String::with_capacity(text.len() + 4 * tab);
    let mut offsets = Vec::with_capacity(text.len() + 1);
    let mut col = 0;
    for c in text.chars() {
        let at = out.len() as u32;
        if c == '\t' {
            offsets.push(at);
            let next = (col / tab + 1) * tab;
            out.extend(std::iter::repeat_n(' ', next - col));
            col = next;
        } else {
            // Bytes inside a multi-byte char shift with it, so they stay off char boundaries.
            offsets.extend((0..c.len_utf8() as u32).map(|k| at + k));
            out.push(c);
            col += 1;
        }
    }
    offsets.push(out.len() as u32);
    Expanded { text: Cow::Owned(out), offsets: Some(offsets) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_stop_at_tab_stops() {
        assert_eq!(columns("a\tb", 4), 5);
        assert_eq!(columns("\t\tx", 4), 9);
        assert_eq!(columns("ab\tc", 2), 5);
        assert_eq!(columns("é\t", 4), 4);
        assert_eq!(columns("a\tb", 0), columns("a\tb", 1));
        assert_eq!(columns("a\tb", 1), 3);
        assert_eq!(columns("", 4), 0);
    }

    #[test]
    fn no_tabs_borrows_and_maps_identity() {
        let e = expand("abc", 4);
        assert!(matches!(e.text, Cow::Borrowed("abc")));
        assert_eq!((e.map(0), e.map(2), e.map(3)), (0, 2, 3));
    }

    #[test]
    fn tabs_expand_to_the_next_stop() {
        let e = expand("a\tb", 4);
        assert_eq!(e.text, "a   b");
        assert_eq!([e.map(0), e.map(1), e.map(2), e.map(3)], [0, 1, 4, 5]);
        assert_eq!(e.map(99), u32::MAX);
        assert_eq!(expand("\t\tx", 4).text, "        x");
        assert_eq!(expand("ab\tc", 2).text, "ab  c");
        assert_eq!(expand("a\tb", 0).text, "a b");
    }

    #[test]
    fn bytes_inside_a_char_stay_off_boundaries() {
        let e = expand("é\tx", 4);
        assert_eq!(e.text, "é   x");
        let inside = e.map(1) as usize;
        assert!(!e.text.is_char_boundary(inside));
        assert_eq!(e.map(2), 2, "the tab starts after é");
        assert_eq!(e.map(3), 5, "x after three spaces");
    }
}
