//! Reading one ssh config file's text (ssh_config(5)): its lines split into a keyword and
//! arguments the way OpenSSH splits them, and the lines Trellis needs from it: the `Host` and
//! `Match` lines that start sections, `IdentityFile` and `Include`. Pure.

/// One line of a config file that Trellis reads.
#[derive(Clone, Debug, PartialEq)]
pub enum Directive {
    /// `Host …` or `Match …`: the arguments joined by one space, after the keyword in its usual
    /// spelling ("Host github.com *.corp").
    Section { line: usize, text: String },
    /// `IdentityFile <raw>`: its first argument, unexpanded. `none` is left out.
    Identity { line: usize, raw: String },
    /// `Include <patterns>…`.
    Include { line: usize, patterns: Vec<String> },
}

/// The keyword (lowercased) and arguments of one config line. None for a blank line, a comment,
/// or a line with an unbalanced quote (which ssh refuses).
///
/// The keyword ends at whitespace or `=`, followed by optional whitespace, at most one `=` and
/// optional whitespace. The arguments are split as OpenSSH splits them: whitespace separates
/// words, `"…"` and `'…'` quote, a backslash before `'`, `"`, `\` or (outside quotes) a space
/// gives that character and any other backslash stays, and an unquoted `#` at the start of a
/// word ends the line.
pub fn config_words(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let end = line.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(line.len());
    let keyword = line[..end].to_ascii_lowercase();
    let rest = line[end..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest);
    Some((keyword, split_args(rest)?))
}

/// The arguments of a line (see [`config_words`]); None when a quote is unbalanced.
fn split_args(s: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        match chars.peek() {
            None | Some('#') => return Some(out),
            Some(_) => {}
        }
        let mut word = String::new();
        let mut quote: Option<char> = None;
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.peek().copied() {
                    Some(n @ ('\'' | '"' | '\\')) => {
                        word.push(n);
                        chars.next();
                    }
                    Some(' ') if quote.is_none() => {
                        word.push(' ');
                        chars.next();
                    }
                    _ => word.push('\\'),
                },
                c if Some(c) == quote => quote = None,
                '"' | '\'' if quote.is_none() => quote = Some(c),
                c if c.is_whitespace() && quote.is_none() => break,
                c => word.push(c),
            }
        }
        if quote.is_some() {
            return None;
        }
        out.push(word);
    }
}

/// The lines Trellis reads from a config file's `text`, with 1-based line numbers.
pub fn parse_config(text: &str) -> Vec<Directive> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line_no = i + 1;
        let Some((keyword, args)) = config_words(line) else { continue };
        match keyword.as_str() {
            "host" | "match" => {
                let name = if keyword == "host" { "Host" } else { "Match" };
                let text = std::iter::once(name).chain(args.iter().map(String::as_str)).collect::<Vec<_>>().join(" ");
                out.push(Directive::Section { line: line_no, text });
            }
            "identityfile" => {
                if let Some(raw) = args.into_iter().next().filter(|a| !a.is_empty() && !a.eq_ignore_ascii_case("none")) {
                    out.push(Directive::Identity { line: line_no, raw });
                }
            }
            "include" => {
                let patterns: Vec<String> = args.into_iter().filter(|a| !a.is_empty()).collect();
                if !patterns.is_empty() {
                    out.push(Directive::Include { line: line_no, patterns });
                }
            }
            _ => {}
        }
    }
    out
}
