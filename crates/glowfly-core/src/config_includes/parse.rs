//! Reads the include entries out of one git config file's text: `path` keys in `[include]` and
//! `[includeIf "<condition>"]` sections, in file order, everything else skipped. Bytes in,
//! bytes out (a config file need not be UTF-8). It follows git's syntax: section and key names in
//! any case, `[section "sub"]` with `\"` and `\\` escapes, a key on the header's line, `#` and
//! `;` comments, quoted values, `\n` `\t` `\b` escapes and `\` at a line's end continuing the
//! value. A line git would refuse is skipped rather than ending the file.

/// One include entry: its condition (None for `[include]`) and the path as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub condition: Option<Vec<u8>>,
    pub path: Vec<u8>,
}

/// Which section the lines being read belong to.
enum Section {
    Include,
    IncludeIf(Vec<u8>),
    Other,
}

/// The include entries of `text`.
pub(super) fn includes(text: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut section = Section::Other;
    let mut at = 0;
    while at < text.len() {
        let line_start = at;
        skip_space(text, &mut at);
        match text.get(at) {
            None => break,
            Some(b'\n') => at += 1,
            Some(b'#' | b';') => skip_line(text, &mut at),
            Some(b'[') => match header(text, &mut at) {
                Some(s) => section = s,
                None => {
                    section = Section::Other;
                    skip_line(text, &mut at);
                }
            },
            Some(_) => {
                let entry = key_value(text, &mut at);
                if let (Some((key, Some(value))), Section::Include | Section::IncludeIf(_)) = (entry, &section)
                    && key.eq_ignore_ascii_case(b"path")
                    && !value.is_empty()
                {
                    let condition = match &section {
                        Section::IncludeIf(c) => Some(c.clone()),
                        _ => None,
                    };
                    out.push(Entry { condition, path: value });
                }
            }
        }
        if at == line_start {
            skip_line(text, &mut at); // never stuck on a byte nothing above reads
        }
    }
    out
}

fn skip_space(text: &[u8], at: &mut usize) {
    while matches!(text.get(*at), Some(b' ' | b'\t' | b'\r')) {
        *at += 1;
    }
}

fn skip_line(text: &[u8], at: &mut usize) {
    while let Some(&b) = text.get(*at) {
        *at += 1;
        if b == b'\n' {
            break;
        }
    }
}

/// Reads a `[...]` header starting at `at`; leaves `at` after the `]`, where a key may follow.
fn header(text: &[u8], at: &mut usize) -> Option<Section> {
    *at += 1;
    let start = *at;
    while text.get(*at).is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.') {
        *at += 1;
    }
    let name = &text[start..*at];
    let sub = match text.get(*at) {
        Some(b']') => None,
        Some(b' ' | b'\t') => {
            skip_space(text, at);
            if text.get(*at) != Some(&b'"') {
                return None;
            }
            *at += 1;
            let mut sub = Vec::new();
            loop {
                match *text.get(*at)? {
                    b'"' => break,
                    b'\n' => return None,
                    b'\\' => {
                        *at += 1;
                        match *text.get(*at)? {
                            b'\n' => return None,
                            b => sub.push(b),
                        }
                    }
                    b => sub.push(b),
                }
                *at += 1;
            }
            *at += 1;
            if text.get(*at) != Some(&b']') {
                return None;
            }
            Some(sub)
        }
        _ => return None,
    };
    *at += 1;
    Some(match sub {
        None if name.eq_ignore_ascii_case(b"include") => Section::Include,
        Some(c) if name.eq_ignore_ascii_case(b"includeif") => Section::IncludeIf(c),
        _ => Section::Other,
    })
}

/// Reads `key [= value]` starting at `at`, through the end of its (last continued) line. None
/// for a line that isn't one; a key with no `=` has no value.
fn key_value(text: &[u8], at: &mut usize) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
    let start = *at;
    if !text.get(*at).is_some_and(u8::is_ascii_alphabetic) {
        skip_line(text, at);
        return None;
    }
    while text.get(*at).is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'-') {
        *at += 1;
    }
    let key = text[start..*at].to_vec();
    skip_space(text, at);
    match text.get(*at) {
        None => return Some((key, None)),
        Some(b'\n') => {
            *at += 1;
            return Some((key, None));
        }
        Some(b'#' | b';') => {
            skip_line(text, at);
            return Some((key, None));
        }
        Some(b'=') => *at += 1,
        Some(_) => {
            skip_line(text, at);
            return None;
        }
    }
    value(text, at).map(|v| (key, Some(v)))
}

/// Reads a value from `at` to the end of its line, unquoting it; None for one git refuses (an
/// unknown escape, an unclosed quote).
fn value(text: &[u8], at: &mut usize) -> Option<Vec<u8>> {
    skip_space(text, at);
    let mut out = Vec::new();
    let mut quoted = false;
    let mut kept = 0; // the length without the unquoted spaces at its end
    let mut ok = true;
    while let Some(&b) = text.get(*at) {
        *at += 1;
        match b {
            b'\n' if !quoted => break,
            b'\n' => {
                ok = false;
                break;
            }
            b'#' | b';' if !quoted => {
                skip_line(text, at);
                break;
            }
            b' ' | b'\t' if !quoted => out.push(b' '),
            b'\r' if !quoted && text.get(*at) == Some(&b'\n') => {}
            b'"' => {
                quoted = !quoted;
                kept = out.len();
            }
            b'\\' => {
                let Some(&e) = text.get(*at) else { break };
                *at += 1;
                match e {
                    b'\n' => {}
                    b'\r' if text.get(*at) == Some(&b'\n') => *at += 1,
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'b' => {
                        out.pop();
                    }
                    b'\\' | b'"' => out.push(e),
                    _ => ok = false,
                }
                kept = out.len();
            }
            _ => {
                out.push(b);
                kept = out.len();
            }
        }
    }
    if quoted {
        ok = false;
    }
    out.truncate(kept);
    ok.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(text: &str) -> Vec<(Option<String>, String)> {
        let s = |b: &[u8]| String::from_utf8(b.to_vec()).unwrap();
        includes(text.as_bytes()).into_iter().map(|e| (e.condition.as_deref().map(s), s(&e.path))).collect()
    }

    #[test]
    fn include_sections_in_any_case_and_layout() {
        let text = "[core]\n\tpath = no\n[Include]\n\tPATH = a.cfg ; note\n[includeIf \"gitdir:~/w/\"]\npath=\"b c.cfg\"\n\
                    [includeif \"onbranch:x\"] path = d\\\\e  \n[include \"sub\"]\npath = skipped\n";
        let want = [(None, "a.cfg"), (Some("gitdir:~/w/"), "b c.cfg"), (Some("onbranch:x"), "d\\e")];
        assert_eq!(paths(text), want.map(|(c, p)| (c.map(String::from), p.to_owned())));
    }

    #[test]
    fn escapes_continuations_and_comments() {
        let text = "[include]\r\n  path = \"x\\\"y\" # c\r\n  path = long\\\n-name\n  path\n  path =\n# path = commented\n";
        assert_eq!(paths(text), [(None, "x\"y".to_owned()), (None, "long-name".to_owned())]);
        let sub = "[includeIf \"gitdir:a\\\"b\\\\c\"]\npath = p\n";
        assert_eq!(paths(sub), [(Some("gitdir:a\"b\\c".to_owned()), "p".to_owned())]);
    }

    #[test]
    fn broken_lines_are_skipped() {
        let text = "[include\npath = a\n[include]\n= x\npath = \"open\npath = \\q\n\x00\npath = ok\n";
        assert_eq!(paths(text), [(None, "ok".to_owned())]);
    }
}
