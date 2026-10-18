//! Find in the blame view and the file history's File view: the query, the lines that match, and the current match. Matching
//! folds ASCII case only, so a match's byte range in the line is the query's in the text
//! (other characters must match exactly).

use std::ops::Range;

#[derive(Clone, Default, Debug)]
pub struct BlameFind {
    pub query: String,
    /// The lines with a match, in order (0-based).
    pub hits: Vec<usize>,
    /// Index into `hits`.
    pub current: Option<usize>,
}

/// The lines holding `query`; none for an empty query.
pub fn search(lines: &[String], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    lines.iter().enumerate().filter(|(_, l)| find_from(l, query, 0).is_some()).map(|(i, _)| i).collect()
}

/// The byte ranges of `query` in `line`, left to right and not overlapping.
pub fn ranges(line: &str, query: &str) -> Vec<Range<u32>> {
    let mut out = Vec::new();
    if query.is_empty() {
        return out;
    }
    let mut from = 0;
    while let Some(at) = find_from(line, query, from) {
        let end = at + query.len();
        out.push(at as u32..end as u32);
        from = end;
    }
    out
}

/// The first byte offset at or after `from` where `query` matches, ignoring ASCII case.
fn find_from(line: &str, query: &str, from: usize) -> Option<usize> {
    let (h, q) = (line.as_bytes(), query.as_bytes());
    if q.len() > h.len() {
        return None;
    }
    (from..=h.len() - q.len()).find(|&i| h[i..i + q.len()].eq_ignore_ascii_case(q))
}

/// The 0-based line for the 1-based number typed in a Line field, clamped into a file of `n`
/// lines; None for other text or an empty file.
pub fn typed_line(typed: &str, n: usize) -> Option<usize> {
    let typed = typed.trim().parse::<usize>().ok()?;
    (n > 0).then(|| typed.clamp(1, n) - 1)
}

impl BlameFind {
    /// Searches `lines` again for the query and makes the first match at or after `top` (else
    /// the first) current; returns its line.
    pub fn refresh(&mut self, lines: &[String], top: usize) -> Option<usize> {
        self.hits = search(lines, &self.query);
        self.current = None;
        let i = self.hits.iter().position(|&l| l >= top).or((!self.hits.is_empty()).then_some(0))?;
        self.current = Some(i);
        Some(self.hits[i])
    }

    /// Moves the current match by `d` (wrapping round) and returns its line.
    pub fn step(&mut self, d: isize) -> Option<usize> {
        let n = self.hits.len() as isize;
        if n == 0 {
            return None;
        }
        let next = match self.current {
            Some(c) => (c as isize + d).rem_euclid(n),
            None if d < 0 => n - 1,
            None => 0,
        } as usize;
        self.current = Some(next);
        Some(self.hits[next])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ranges` as (start, end) pairs.
    fn spans(line: &str, query: &str) -> Vec<(u32, u32)> {
        ranges(line, query).into_iter().map(|r| (r.start, r.end)).collect()
    }

    #[test]
    fn ranges_fold_ascii_case() {
        assert_eq!(spans("Foo bar FOO", "foo"), [(0, 3), (8, 11)]);
        assert!(ranges("bar", "foo").is_empty());
        assert!(ranges("foo", "").is_empty());
    }

    #[test]
    fn multibyte_text_keeps_byte_offsets() {
        assert_eq!(spans("café Point", "point"), [(6, 11)]);
        assert_eq!(spans("ÉÉ é", "é"), [(5, 7)], "non-ASCII matches exactly");
    }

    #[test]
    fn overlapping_matches_are_one() {
        assert_eq!(spans("aaa", "aa"), [(0, 2)]);
    }

    #[test]
    fn search_and_step_wrap() {
        let lines: Vec<String> = ["foo", "bar", "Foo bar", "baz"].map(String::from).to_vec();
        let mut f = BlameFind { query: "foo".into(), hits: search(&lines, "foo"), current: None };
        assert_eq!(f.hits, [0, 2]);
        assert_eq!(f.step(1), Some(0));
        assert_eq!(f.step(1), Some(2));
        assert_eq!(f.step(1), Some(0));
        assert_eq!(f.step(-1), Some(2));
        assert!(search(&lines, "").is_empty());
        assert_eq!(BlameFind::default().step(1), None);
    }

    #[test]
    fn refresh_picks_the_first_hit_at_or_below_the_top() {
        let lines: Vec<String> = ["foo", "bar", "foo", "baz", "foo"].map(String::from).to_vec();
        let mut f = BlameFind { query: "foo".into(), ..Default::default() };
        assert_eq!(f.refresh(&lines, 1), Some(2));
        assert_eq!((f.hits.as_slice(), f.current), (&[0, 2, 4][..], Some(1)));
        assert_eq!(f.refresh(&lines, 4), Some(4));
        assert_eq!(f.refresh(&lines, 5), Some(0), "past the last hit: the first");
        f.query = "nope".into();
        assert_eq!(f.refresh(&lines, 0), None);
        assert!(f.hits.is_empty() && f.current.is_none());
    }

    #[test]
    fn typed_line_clamps_into_the_file() {
        assert_eq!(typed_line("3", 10), Some(2));
        assert_eq!(typed_line(" 999 ", 10), Some(9));
        assert_eq!(typed_line("0", 10), Some(0));
        assert_eq!(typed_line("abc", 10), None);
        assert_eq!(typed_line("", 10), None);
        assert_eq!(typed_line("3", 0), None);
    }
}
