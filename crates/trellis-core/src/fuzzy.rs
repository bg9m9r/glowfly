//! Fuzzy matching and ranking for the command palette: a query matches a label when its
//! characters appear in order (case-insensitively); matches at word starts and in runs score
//! higher. Pure: no UI and no I/O.

/// A successful match: its score (higher is better) and the matched characters' positions.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub score: i32,
    /// Char indexes into the label.
    pub positions: Vec<usize>,
}

const MATCH: i32 = 16;
const WORD_START: i32 = 16;
const CONSECUTIVE: i32 = 24;
const MAX_LEADING_GAP: i32 = 10;
const MAX_QUERY: usize = 64;
/// How much lower a match through an entry's hidden keywords ranks than one in its label.
const KEYWORD_PENALTY: i32 = 30;
const NONE: i32 = i32::MIN;

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Whether a word starts at char `j` of `text`: the first char, one after a non-alphanumeric
/// char, or a lower-to-upper camel-case boundary.
fn word_start(text: &[char], j: usize) -> bool {
    if j == 0 {
        return true;
    }
    let (prev, cur) = (text[j - 1], text[j]);
    !prev.is_alphanumeric() || (prev.is_lowercase() && cur.is_uppercase())
}

/// Case-insensitive subsequence match of `query` (whitespace in the query is ignored) in `text`.
pub fn fuzzy_match(query: &str, text: &str) -> Option<Match> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(lower).take(MAX_QUERY).collect();
    if q.is_empty() {
        return Some(Match { score: 0, positions: Vec::new() });
    }
    let orig: Vec<char> = text.chars().collect();
    let t: Vec<char> = orig.iter().map(|&c| lower(c)).collect();
    let (m, n) = (q.len(), t.len());
    if m > n {
        return None;
    }
    let bonus: Vec<i32> = (0..n).map(|j| MATCH + if word_start(&orig, j) { WORD_START } else { 0 }).collect();
    // score[i][j]: the best score with query char i matched at text position j; back[i][j]: the
    // position query char i - 1 was matched at on that path.
    let mut score = vec![NONE; m * n];
    let mut back = vec![0usize; m * n];
    for j in 0..n {
        if t[j] == q[0] {
            score[j] = bonus[j] - (j as i32).min(MAX_LEADING_GAP);
        }
    }
    for i in 1..m {
        let (prev, cur) = (i - 1, i);
        // The best `score[prev][k] + k` over k <= j - 2, earliest k on ties: a gap of j - k - 1
        // skipped chars costs that many points.
        let mut best_gap: Option<(i32, usize)> = None;
        for j in i..n {
            if j >= 2 {
                let k = j - 2;
                let s = score[prev * n + k];
                if s != NONE && best_gap.is_none_or(|(b, _)| s + k as i32 > b) {
                    best_gap = Some((s + k as i32, k));
                }
            }
            if t[j] != q[cur] {
                continue;
            }
            let mut best: Option<(i32, usize)> = best_gap.map(|(b, k)| (b + 1 - j as i32, k));
            let adjacent = score[prev * n + j - 1];
            if adjacent != NONE {
                let s = adjacent + CONSECUTIVE;
                if best.is_none_or(|(b, _)| s > b) {
                    best = Some((s, j - 1));
                }
            }
            if let Some((s, k)) = best {
                score[cur * n + j] = s + bonus[j];
                back[cur * n + j] = k;
            }
        }
    }
    let last = (m - 1) * n;
    let (mut best, mut end) = (NONE, 0);
    for j in 0..n {
        if score[last + j] > best {
            (best, end) = (score[last + j], j);
        }
    }
    if best == NONE {
        return None;
    }
    let mut positions = vec![0; m];
    let mut j = end;
    for i in (0..m).rev() {
        positions[i] = j;
        j = back[i * n + j];
    }
    Some(Match { score: best, positions })
}

/// Ranks items given as (label, keywords): matches only, best first. A match in the label
/// beats one that needs the keywords; ties go to the shorter label, then the earlier item.
pub fn rank(query: &str, items: &[(&str, &str)]) -> Vec<(usize, Match)> {
    if query.chars().all(char::is_whitespace) {
        return (0..items.len()).map(|i| (i, Match { score: 0, positions: Vec::new() })).collect();
    }
    let mut found: Vec<(usize, Match, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, (label, keywords))| {
            let len = label.chars().count();
            let m = fuzzy_match(query, label).or_else(|| {
                let mut m = fuzzy_match(query, &format!("{label} {keywords}"))?;
                m.score -= KEYWORD_PENALTY;
                m.positions.retain(|&p| p < len);
                Some(m)
            })?;
            Some((i, m, len))
        })
        .collect();
    found.sort_by(|a, b| b.1.score.cmp(&a.1.score).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    found.into_iter().map(|(i, m, _)| (i, m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(query: &str, items: &[(&str, &str)]) -> Vec<usize> {
        rank(query, items).into_iter().map(|(i, _)| i).collect()
    }

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_match("", "Push"), Some(Match { score: 0, positions: vec![] }));
        assert_eq!(fuzzy_match("  ", "Push"), Some(Match { score: 0, positions: vec![] }));
        let items = [("Open repository…", ""), ("Pull", ""), ("Push", "upload")];
        assert_eq!(order("", &items), vec![0, 1, 2], "input order, not shortest first");
        assert!(rank("", &items).iter().all(|(_, m)| m.score == 0 && m.positions.is_empty()));
    }

    #[test]
    fn non_subsequence_and_too_long_queries_do_not_match() {
        assert_eq!(fuzzy_match("xyz", "Fetch all remotes"), None);
        assert_eq!(fuzzy_match("pushh", "Push"), None);
        assert_eq!(fuzzy_match("hsup", "Push"), None, "order matters");
    }

    #[test]
    fn matching_is_case_insensitive_with_char_positions() {
        assert_eq!(fuzzy_match("FET", "Fetch all remotes").unwrap().positions, vec![0, 1, 2]);
        assert_eq!(fuzzy_match("fet", "FETCH").unwrap().positions, vec![0, 1, 2]);
        // Char indexes, not byte offsets: "Ö" takes two bytes.
        assert_eq!(fuzzy_match("ffn", "Öffnen").unwrap().positions, vec![1, 2, 3]);
        assert_eq!(fuzzy_match("öf", "Öffnen").unwrap().positions, vec![0, 1]);
    }

    #[test]
    fn spaces_in_query_are_ignored() {
        let m = fuzzy_match("co main", "Checkout main").unwrap();
        assert_eq!(m.positions.first(), Some(&0));
        assert_eq!(m.positions.len(), 6);
        assert_eq!(m.positions[2..], [9, 10, 11, 12]);
    }

    #[test]
    fn word_starts_rank_first() {
        assert_eq!(order("sa", &[("Unstage all files", ""), ("Stage all files", "")]), vec![1, 0]);
        // A camel-case boundary counts as a word start.
        assert!(fuzzy_match("b", "openBranch").unwrap().score > fuzzy_match("b", "openbranch").unwrap().score);
    }

    #[test]
    fn consecutive_runs_beat_word_jumps() {
        assert_eq!(order("log", &[("Checkout long-gone-branch", ""), ("Checkout login", "")]), vec![1, 0]);
    }

    #[test]
    fn ties_prefer_shorter_then_earlier() {
        assert_eq!(order("p", &[("Open repository…", ""), ("Push", ""), ("Preferences", "")]), vec![1, 2, 0]);
        assert_eq!(order("t", &[("Tab", ""), ("Tag", "")]), vec![0, 1]);
        assert_eq!(order("t", &[("Tag", ""), ("Tab", "")]), vec![0, 1]);
    }

    #[test]
    fn keywords_match_below_label_matches() {
        assert_eq!(order("theme", &[("Open Preferences", "appearance theme"), ("Theme: Dark", "")]), vec![1, 0]);
        let ranked = rank("settings", &[("Open Preferences", "settings options")]);
        assert_eq!(ranked.len(), 1);
        let len = "Open Preferences".chars().count();
        assert!(ranked[0].1.positions.iter().all(|&p| p < len));
        assert!(rank("zebra", &[("Open Preferences", "settings options")]).is_empty());
    }

    #[test]
    fn long_queries_are_truncated() {
        let text = "a".repeat(100);
        let query = "a".repeat(80);
        assert_eq!(fuzzy_match(&query, &text).unwrap().positions.len(), MAX_QUERY);
    }
}
