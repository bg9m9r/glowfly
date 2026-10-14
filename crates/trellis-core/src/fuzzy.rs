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

/// Whether the (lowercased) query chars appear in order in `text`, ignoring case.
fn is_subsequence(q: &[char], text: impl IntoIterator<Item = char>) -> bool {
    let mut want = q.iter().peekable();
    for c in text {
        match want.peek() {
            Some(&&w) if lower(c) == w => {
                want.next();
            }
            Some(_) => {}
            None => break,
        }
    }
    want.peek().is_none()
}

/// The query as matched: whitespace dropped, lowercased, at most `MAX_QUERY` chars.
fn prepare(query: &str) -> Vec<char> {
    query.chars().filter(|c| !c.is_whitespace()).map(lower).take(MAX_QUERY).collect()
}

/// Case-insensitive subsequence match of `query` (whitespace in the query is ignored) in `text`.
pub fn fuzzy_match(query: &str, text: &str) -> Option<Match> {
    match_prepared(&prepare(query), text)
}

/// `fuzzy_match` for a query already through `prepare`.
fn match_prepared(q: &[char], text: &str) -> Option<Match> {
    if q.is_empty() {
        return Some(Match { score: 0, positions: Vec::new() });
    }
    // Most texts in a long list don't contain the query at all: rule them out in one pass
    // before allocating the score tables.
    if !is_subsequence(q, text.chars()) {
        return None;
    }
    best_match(q, text)
}

/// The best-scoring placement of the lowercased, non-empty query `q` in `text`, if any.
fn best_match(q: &[char], text: &str) -> Option<Match> {
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
    let q = prepare(query);
    let mut found: Vec<(usize, Match, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, (label, keywords))| {
            let len = label.chars().count();
            let m = match_prepared(&q, label).or_else(|| {
                if keywords.is_empty() {
                    return None;
                }
                // Rule out the label and keywords together before building their text: most
                // entries in a long list match neither.
                let joined = label.chars().chain(std::iter::once(' ')).chain(keywords.chars());
                if !is_subsequence(&q, joined) {
                    return None;
                }
                let mut m = best_match(&q, &format!("{label} {keywords}"))?;
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
    fn precheck_keeps_results() {
        let pairs = [
            ("co main", "Checkout main", true),
            ("hsup", "Push", false),
            ("FET", "Fetch all remotes", true),
            ("ffn", "Öffnen", true),
            ("file42.r", "src/mod42/file42.rs", true),
            ("file42", "src/mod41/file4.rs", false),
            ("xyz", "Fetch all remotes", false),
            ("ss", "s", false),
        ];
        for (query, text, matches) in pairs {
            let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(lower).collect();
            let got = fuzzy_match(query, text);
            assert_eq!(got.is_some(), matches, "{query:?} in {text:?}");
            // The same result, score and positions as the full matcher without the precheck.
            assert_eq!(got, best_match(&q, text), "{query:?} in {text:?}");
        }
    }

    /// The ranking before the keyword precheck: the label, else the label and keywords joined.
    fn rank_unchecked(query: &str, label: &str, keywords: &str) -> Option<Match> {
        let len = label.chars().count();
        fuzzy_match(query, label).or_else(|| {
            if keywords.is_empty() {
                return None;
            }
            let mut m = fuzzy_match(query, &format!("{label} {keywords}"))?;
            m.score -= KEYWORD_PENALTY;
            m.positions.retain(|&p| p < len);
            Some(m)
        })
    }

    #[test]
    fn rank_keyword_precheck_matches_concatenation() {
        let cases = [
            ("push", "Push", "upload publish"),
            ("upload", "Push", "upload publish"),
            ("pushup", "Push", "upload publish"),
            ("tagdet", "Checkout tag v1.0", "switch tag release version detached"),
            ("v1 rel", "Checkout tag v1.0", "switch tag release version detached"),
            ("zebra", "Open Preferences", "settings options"),
            ("settings", "Open Preferences", ""),
            ("Prefs", "Open Preferences", ""),
            ("   ", "Open Preferences", "settings"),
            ("hsup", "Push", "upload"),
            ("öfopt", "Öffnen", "options"),
        ];
        for (query, label, keywords) in cases {
            let got = rank(query, &[(label, keywords)]).into_iter().next().map(|(_, m)| m);
            let want = if query.trim().is_empty() {
                Some(Match { score: 0, positions: Vec::new() })
            } else {
                rank_unchecked(query, label, keywords)
            };
            assert_eq!(got, want, "{query:?} in {label:?} / {keywords:?}");
        }
        // The cases cover a label hit, a keyword-only hit, a hit across both and a miss.
        assert!(fuzzy_match("upload", "Push").is_none() && rank_unchecked("upload", "Push", "upload publish").is_some());
        assert!(fuzzy_match("pushup", "Push").is_none() && rank_unchecked("pushup", "Push", "upload publish").is_some());
        assert!(rank_unchecked("zebra", "Open Preferences", "settings options").is_none());
    }

    #[test]
    fn rank_many_items() {
        let labels: Vec<String> = (0..20_000).map(|i| format!("Checkout tag v{i}")).collect();
        let items: Vec<(&str, &str)> = labels.iter().map(|l| (l.as_str(), "switch tag release version detached")).collect();
        let found = rank("v19999", &items);
        assert_eq!(found.first().map(|(i, _)| *i), Some(19_999));
        assert!(rank("zzzz", &items).is_empty());
        assert_eq!(rank("", &items).len(), 20_000);
    }

    #[test]
    fn long_queries_are_truncated() {
        let text = "a".repeat(100);
        let query = "a".repeat(80);
        assert_eq!(fuzzy_match(&query, &text).unwrap().positions.len(), MAX_QUERY);
    }
}
