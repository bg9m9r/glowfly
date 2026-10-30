//! A config program's command split into words as sh splits it, for `programs.rs`: quotes
//! taken off, a `\` outside quotes keeping the next character (an escaped space too), and each
//! substitution or expansion (`$(…)`, a backtick one, `${…}`, `$NAME`), whatever is inside it
//! and however it is spaced, kept as one `SUB` mark. A glob character in quotes is kept as a
//! stand-in (`literal` gives it back), so only an unquoted one reads as a glob.

/// Where a substitution or expansion stood: its value is unknown (the working folder, when a
/// `/` follows it at the start of a word).
/// Private-use code points: never drawn, and never in a setting a person types.
pub(super) const SUB: char = private(0);

/// Stand-ins for glob and bracket characters that were quoted, so they are no glob.
const QUOTED: [(char, char); 6] =
    [('*', private(1)), ('?', private(2)), ('[', private(3)), (']', private(4)), ('{', private(5)), ('}', private(6))];

/// The private-use character `n` places into the BMP's private-use area.
const fn private(n: u32) -> char {
    match char::from_u32(0xE000 + n) {
        Some(c) => c,
        None => '\0',
    }
}

/// One word, as sh reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Word {
    /// sh's reading: escapes applied.
    pub sh: String,
    /// The word with each `\` outside quotes kept before a character other than a space, as a
    /// program on Windows given it would read it (a folder separator); None when it had none.
    pub raw: Option<String>,
}

/// `text` with the quoted stand-ins put back.
pub(super) fn literal(text: &str) -> String {
    text.chars().map(|c| QUOTED.iter().find(|(_, s)| *s == c).map_or(c, |(q, _)| *q)).collect()
}

/// The words of `text`, as sh splits them.
pub(super) fn words(text: &str) -> Vec<Word> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let (mut sh, mut raw, mut escaped) = (String::new(), String::new(), false);
    let mut quote: Option<char> = None;
    let mut i = 0;
    let quoted = |c: char| QUOTED.iter().find(|(q, _)| *q == c).map_or(c, |(_, s)| *s);
    while i < chars.len() {
        let c = chars[i];
        if let Some(end) = (c == '$' || c == '`').then(|| substitution(&chars, i)).flatten() {
            sh.push(SUB);
            raw.push(SUB);
            i = end;
            continue;
        }
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !sh.is_empty() {
                    out.push(Word { sh: std::mem::take(&mut sh), raw: escaped.then(|| raw.clone()) });
                }
                raw.clear();
                escaped = false;
            }
            (None | Some('"'), '\\') if i + 1 < chars.len() => {
                let next = chars[i + 1];
                // In double quotes a `\` escapes only these; before anything else it stays.
                if quote.is_some() && !matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                    sh.push('\\');
                    raw.push('\\');
                    i += 1;
                    continue;
                }
                sh.push(quoted(next));
                if !next.is_whitespace() {
                    raw.push('\\');
                    escaped = true;
                }
                raw.push(quoted(next));
                i += 2;
                continue;
            }
            (q, c) => {
                let c = if q.is_some() { quoted(c) } else { c };
                sh.push(c);
                raw.push(c);
            }
        }
        i += 1;
    }
    if !sh.is_empty() {
        out.push(Word { sh, raw: escaped.then_some(raw) });
    }
    out
}

/// Where the substitution or expansion starting at `chars[at]` (a `$` or a backtick) ends, or
/// None when it is none (a lone `$`).
fn substitution(chars: &[char], at: usize) -> Option<usize> {
    let closing = |open: char, close: char, from: usize| {
        let mut depth = 0usize;
        for (i, c) in chars.iter().enumerate().skip(from) {
            if *c == open {
                depth += 1;
            } else if *c == close {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
        }
        chars.len()
    };
    if chars[at] == '`' {
        return Some(chars.iter().skip(at + 1).position(|c| *c == '`').map_or(chars.len(), |p| at + p + 2));
    }
    match chars.get(at + 1) {
        Some('(') => Some(closing('(', ')', at + 1)),
        Some('{') => Some(closing('{', '}', at + 1)),
        Some(c) if c.is_ascii_alphabetic() || *c == '_' => {
            let len = chars[at + 1..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == '_').count();
            Some(at + 1 + len)
        }
        Some(c) if c.is_ascii_digit() || "@*#?$!-".contains(*c) => Some(at + 2),
        _ => None,
    }
}
