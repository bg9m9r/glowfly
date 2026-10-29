//! Git's pattern matching for include conditions, on bytes: `?` and `*` never match a `/`, `**`
//! between slashes (or at either end) matches across folders, `[...]` is a set of bytes
//! (`!` or `^` negates it, `a-z` ranges, `[:alpha:]` classes), and `\` makes the next byte
//! literal. `fold` compares ASCII letters in any case (`gitdir/i:`).
//!
//! Backtracking is cut short the way git's matcher does it: once a `*` has tried the rest of the
//! text, a failure says whether no later start can match either, so each star tries each start
//! at most once and a long pattern of stars stays linear in practice.

/// What a match attempt found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Found {
    Match,
    NoMatch,
    /// The text ran out: no later start of an enclosing star can match.
    GiveUp,
    /// A single star reached a `/`: only an enclosing `**` may try further.
    GiveUpToDoubleStar,
}

/// Whether the whole of `text` matches `pattern`.
pub(super) fn matches(pattern: &[u8], text: &[u8], fold: bool) -> bool {
    walk(pattern, text, fold) == Found::Match
}

fn same(a: u8, b: u8, fold: bool) -> bool {
    a == b || (fold && a.eq_ignore_ascii_case(&b))
}

fn walk(pattern: &[u8], text: &[u8], fold: bool) -> Found {
    let (mut p, mut t) = (0, 0);
    while p < pattern.len() {
        let c = pattern[p];
        if t >= text.len() && c != b'*' {
            return Found::GiveUp;
        }
        match c {
            b'\\' if p + 1 < pattern.len() => {
                p += 1;
                if !same(pattern[p], text[t], fold) {
                    return Found::NoMatch;
                }
            }
            b'?' => {
                if text[t] == b'/' {
                    return Found::NoMatch;
                }
            }
            b'*' => return star(pattern, p, &text[t..], fold),
            b'[' => match set(&pattern[p + 1..], text[t], fold) {
                Some((true, used)) => p += used,
                Some((false, _)) => return Found::NoMatch,
                // An unclosed `[` never matches, as in git.
                None => return Found::GiveUp,
            },
            _ => {
                if !same(c, text[t], fold) {
                    return Found::NoMatch;
                }
            }
        }
        p += 1;
        t += 1;
    }
    if t == text.len() { Found::Match } else { Found::NoMatch }
}

/// Matches from the star at `pattern[at]` against `text` (what is left of it).
fn star(pattern: &[u8], at: usize, text: &[u8], fold: bool) -> Found {
    let mut p = at + 1;
    let mut across = false;
    if pattern.get(p) == Some(&b'*') {
        while pattern.get(p) == Some(&b'*') {
            p += 1;
        }
        let after_slash = at == 0 || pattern[at - 1] == b'/';
        let before_slash = p == pattern.len() || pattern[p] == b'/' || (pattern[p] == b'\\' && pattern.get(p + 1) == Some(&b'/'));
        if after_slash && before_slash {
            // `**/` also matches no folder at all.
            if pattern.get(p) == Some(&b'/') && walk(&pattern[p + 1..], text, fold) == Found::Match {
                return Found::Match;
            }
            across = true;
        }
    }
    let rest = &pattern[p..];
    if rest.is_empty() {
        return if !across && text.contains(&b'/') { Found::GiveUpToDoubleStar } else { Found::Match };
    }
    if !across && rest[0] == b'/' {
        // One star then a slash: it can only end at the text's next slash.
        let Some(slash) = text.iter().position(|&b| b == b'/') else { return Found::GiveUp };
        return walk(rest, &text[slash..], fold);
    }
    for start in 0..=text.len() {
        match walk(rest, &text[start..], fold) {
            Found::NoMatch => {}
            Found::GiveUpToDoubleStar if across => {}
            other => return other,
        }
        if !across && text.get(start) == Some(&b'/') {
            return Found::GiveUpToDoubleStar;
        }
    }
    Found::GiveUp
}

/// Matches `byte` against the set whose text starts at `body` (just after the `[`): whether it
/// matched, and how many pattern bytes the body and its `]` take. None when the set never ends.
fn set(body: &[u8], byte: u8, fold: bool) -> Option<(bool, usize)> {
    let mut i = 0;
    let negated = matches!(body.first(), Some(b'!' | b'^'));
    if negated {
        i += 1;
    }
    let mut hit = false;
    let mut first = true;
    loop {
        let c = *body.get(i)?;
        if c == b']' && !first {
            break;
        }
        first = false;
        if c == b'[' && body.get(i + 1) == Some(&b':') {
            let end = body[i + 2..].windows(2).position(|w| w == b":]")?;
            let name = &body[i + 2..i + 2 + end];
            hit |= class(name, byte, fold);
            i += end + 4;
            continue;
        }
        let low = if c == b'\\' {
            i += 1;
            *body.get(i)?
        } else {
            c
        };
        if body.get(i + 1) == Some(&b'-') && body.get(i + 2).is_some_and(|&n| n != b']') {
            let mut high = body[i + 2];
            i += 2;
            if high == b'\\' {
                i += 1;
                high = *body.get(i)?;
            }
            hit |= (low..=high).contains(&byte)
                || (fold && ((low..=high).contains(&byte.to_ascii_lowercase()) || (low..=high).contains(&byte.to_ascii_uppercase())));
        } else {
            hit |= same(low, byte, fold);
        }
        i += 1;
    }
    // A set never matches a slash.
    Some((hit != negated && byte != b'/', i + 1))
}

fn class(name: &[u8], b: u8, fold: bool) -> bool {
    match name {
        b"alnum" => b.is_ascii_alphanumeric(),
        b"alpha" => b.is_ascii_alphabetic(),
        b"blank" => b == b' ' || b == b'\t',
        b"cntrl" => b.is_ascii_control(),
        b"digit" => b.is_ascii_digit(),
        b"graph" => b.is_ascii_graphic(),
        b"lower" => b.is_ascii_lowercase() || (fold && b.is_ascii_uppercase()),
        b"print" => b.is_ascii_graphic() || b == b' ',
        b"punct" => b.is_ascii_punctuation(),
        b"space" => b.is_ascii_whitespace() || b == 0x0b,
        b"upper" => b.is_ascii_uppercase() || (fold && b.is_ascii_lowercase()),
        b"xdigit" => b.is_ascii_hexdigit(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::matches;

    fn m(p: &str, t: &str) -> bool {
        matches(p.as_bytes(), t.as_bytes(), false)
    }

    #[test]
    fn stars_and_double_stars() {
        assert!(m("**/proj/**", "/home/u/proj/.git"));
        assert!(m("/home/*/proj/.git", "/home/u/proj/.git"));
        assert!(!m("/home/*/.git", "/home/u/proj/.git"), "one star stays in its folder");
        assert!(m("/home/**/.git", "/home/u/proj/.git"));
        assert!(m("/home/**/.git", "/home/.git"), "`**/` matches no folder too");
        assert!(m("a**b", "axxb") && !m("a**b", "a/b"), "`**` inside a name is a single star");
        assert!(m("feature/**", "feature/x/y") && !m("feature/*", "feature/x/y"));
        assert!(m("*", "main") && !m("*", "a/b"));
        assert!(!m("**/x", "") && m("**", ""));
    }

    #[test]
    fn sets_escapes_and_case() {
        assert!(m("[ab]c", "bc") && !m("[!ab]c", "bc") && m("[^ab]c", "cc"));
        assert!(m("v[0-9]", "v7") && !m("v[0-9]", "vx"));
        assert!(m("[[:upper:]]x", "Ax") && !m("[[:upper:]]x", "ax"));
        assert!(m("a\\*", "a*") && !m("a\\*", "ab"));
        assert!(!m("a[/]b", "a/b"), "a set never matches a slash");
        assert!(!m("a[b", "a[b"), "an unclosed set matches nothing");
        assert!(matches(b"/Work/**", b"/work/x", true) && !matches(b"/Work/**", b"/work/x", false));
    }

    #[test]
    fn many_stars_finish() {
        let text = "a".repeat(60);
        assert!(!m(&format!("{}b", "*a".repeat(30)), &text));
    }
}
