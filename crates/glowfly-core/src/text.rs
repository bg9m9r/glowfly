//! Making text from outside (servers, remote URLs) safe to show or log, and git command lines as
//! typed for the job log.

/// Unicode bidirectional formatting characters, which can make text display in a different
/// order than it reads (e.g. hide the real end of a URL or file name).
pub fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Characters drawn as nothing at all (zero-width spaces and joiners, word joiners, invisible
/// operators, the byte order mark, soft hyphens, Hangul fillers and the other default-ignorable
/// format characters): they can hide inside a name or an address without showing.
pub fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200D}'
            | '\u{2060}'..='\u{2065}'
            | '\u{206A}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

/// `s` with control and bidi characters removed and runs of whitespace collapsed to one space.
pub fn printable(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if c.is_control() || is_bidi_control(c) || is_invisible(c) {
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    out
}

/// `s` with its lines kept (`\r\n` and a lone `\r` read as `\n`) and tabs kept, but every other
/// control character, every bidi character and every invisible one (`is_invisible`) removed: for
/// multi-line text such as a description.
pub fn printable_lines(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    s.chars().filter(|&c| matches!(c, '\n' | '\t') || !(c.is_control() || is_bidi_control(c) || is_invisible(c))).collect()
}

/// `text` with the user/password part of every URL in it replaced by `***`
/// (`https://user:token@host/x` → `https://***@host/x`), for echoing commands into logs. A
/// plain `ssh://git@host` user name is kept.
pub fn redact_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        // Just after the last character that can't be in a scheme (which may take several bytes).
        let scheme_start = rest[..i]
            .char_indices()
            .rfind(|&(_, c)| !(c.is_ascii_alphanumeric() || "+-.".contains(c)))
            .map_or(0, |(p, c)| p + c.len_utf8());
        let scheme = rest[scheme_start..i].to_ascii_lowercase();
        let after = &rest[i + 3..];
        let end = after.find(|c: char| "/?#".contains(c) || c.is_whitespace()).unwrap_or(after.len());
        out.push_str(&rest[..i + 3]);
        match after[..end].rfind('@') {
            Some(at) if scheme.starts_with("http") || scheme.starts_with("ftp") || after[..at].contains(':') => {
                out.push_str("***");
                out.push_str(&after[at..end]);
            }
            _ => out.push_str(&after[..end]),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// `a` as it is typed in a command line: unchanged unless it is empty or holds whitespace or a
/// `"`, which puts it in double quotes with `\` and `"` escaped and a line break written `\n`
/// (`\r` for a carriage return).
fn typed_arg(a: &str) -> std::borrow::Cow<'_, str> {
    if !a.is_empty() && !a.chars().any(|c| c.is_whitespace() || c == '"') {
        return std::borrow::Cow::Borrowed(a);
    }
    let mut out = String::with_capacity(a.len() + 2);
    out.push('"');
    for c in a.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    std::borrow::Cow::Owned(out)
}

/// `git` and `args` as typed: an argument that is empty or holds whitespace or `"` in double
/// quotes with `\` and `"` escaped, a line break inside one written `\n` (`\r` for a carriage
/// return), so the line stays one line. Not redacted: a job log uses `command_echo`.
pub fn command_line(args: &[String]) -> String {
    let mut out = String::from("git");
    for a in args {
        out.push(' ');
        out.push_str(&typed_arg(a));
    }
    out
}

/// A job log's echo of `git args` (`$ git …`, one line): `command_line` with URL credentials
/// hidden (`redact_credentials` on each argument before quoting, so a URL after a line break
/// inside an argument is still found). Every `$ git` echo is built here
/// (tests/source_checks/command_echo.rs).
pub fn command_echo(args: &[String]) -> String {
    let redacted: Vec<String> = args.iter().map(|a| redact_credentials(a)).collect();
    format!("$ {}", command_line(&redacted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_drops_control_and_bidi_characters() {
        assert_eq!(printable("  Bad\tcredentials\r\n\x1b[31mred\u{202e}txt.exe \u{2066}x "), "Bad credentials [31mredtxt.exe x");
        assert_eq!(printable("plain"), "plain");
        assert!(is_bidi_control('\u{202E}') && !is_bidi_control('a'));
    }

    #[test]
    fn printable_lines_keeps_lines_and_tabs_only() {
        assert_eq!(printable_lines("a\r\nb\rc\n\tcode\x1b[0m\u{202e}gpj.exe\u{7}"), "a\nb\nc\n\tcode[0mgpj.exe");
        assert_eq!(printable_lines("  keeps  spaces "), "  keeps  spaces ");
    }

    #[test]
    fn invisible_characters_are_dropped() {
        for c in ['\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{2061}', '\u{2064}', '\u{FEFF}', '\u{00AD}', '\u{E0041}'] {
            let s = format!("github{c}.com");
            assert_eq!(printable(&s), "github.com", "{:04X}", c as u32);
            assert_eq!(printable_lines(&format!("a\n{s}")), "a\ngithub.com", "{:04X}", c as u32);
        }
        assert!(!is_invisible('a') && !is_invisible(' ') && !is_invisible('\u{FF0E}'));
    }

    #[test]
    fn redacts_credentials_in_urls() {
        let r = redact_credentials;
        assert_eq!(r("push https://user:TOKEN@host/o/r.git main"), "push https://***@host/o/r.git main");
        assert_eq!(r("https://ghp_secret@github.com/o/r"), "https://***@github.com/o/r");
        assert_eq!(r("remote.o.url=HTTP://a:b@h:8080/x"), "remote.o.url=HTTP://***@h:8080/x");
        assert_eq!(r("ssh://u:pw@h/x and https://h/y"), "ssh://***@h/x and https://h/y");
        for same in ["ssh://git@h/x", "git@h:o/r", "https://h/o@r", "plain text", "C:/repos/x", "https://"] {
            assert_eq!(r(same), same);
        }
    }

    #[test]
    fn a_character_of_several_bytes_before_a_url_is_kept_whole() {
        let r = redact_credentials;
        assert_eq!(r("«https://user:TOKEN@host/x»"), "«https://***@host/x»");
        assert_eq!(r("é://u:p@h/x"), "é://***@h/x");
        assert_eq!(r("\u{fffd}to://m"), "\u{fffd}to://m", "found by the redact_credentials fuzz check");
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn command_line_writes_plain_arguments_as_they_are() {
        assert_eq!(command_line(&args(&["push", "--", "origin", "main"])), "git push -- origin main");
        assert_eq!(command_line(&[]), "git");
        assert_eq!(command_line(&args(&["worktree", "remove", r"C:\x", "stash@{0}"])), r"git worktree remove C:\x stash@{0}");
    }

    #[test]
    fn command_line_quotes_an_argument_with_a_space() {
        assert_eq!(
            command_line(&args(&["merge", "-m", "Merge branch 'x'", "--", "refs/heads/x"])),
            r#"git merge -m "Merge branch 'x'" -- refs/heads/x"#
        );
        assert_eq!(command_line(&args(&["x", "a\tb"])), "git x \"a\tb\"");
    }

    #[test]
    fn command_line_quotes_an_empty_argument() {
        assert_eq!(command_line(&args(&["config", "x.y", ""])), r#"git config x.y """#);
    }

    #[test]
    fn command_line_escapes_quotes_and_backslashes() {
        assert_eq!(command_line(&args(&["commit", "-m", r#"say "hi" in C:\a b"#])), r#"git commit -m "say \"hi\" in C:\\a b""#);
        assert_eq!(command_line(&args(&["x", r#"a"b"#])), r#"git x "a\"b""#);
    }

    #[test]
    fn command_line_keeps_a_line_break_on_one_line() {
        let line = command_line(&args(&["commit", "-m", "one\ntwo\r\n"]));
        assert_eq!(line, r#"git commit -m "one\ntwo\r\n""#);
        assert!(!line.contains('\n') && !line.contains('\r'));
        // A typed backslash and n stays apart from an escaped line break.
        assert_eq!(command_line(&args(&["x", r"a\n b"])), r#"git x "a\\n b""#);
    }

    #[test]
    fn command_echo_hides_credentials() {
        assert_eq!(command_echo(&args(&["push", "https://u:TOKEN@h/o/r.git", "main"])), "$ git push https://***@h/o/r.git main");
        assert_eq!(
            command_echo(&args(&["remote", "add", "my fork", "https://u:TOKEN@h/r"])),
            r#"$ git remote add "my fork" https://***@h/r"#
        );
        assert_eq!(command_echo(&args(&["-c", "remote.o.url=x y https://a:b@h/x"])), r#"$ git -c "remote.o.url=x y https://***@h/x""#);
    }

    #[test]
    fn command_echo_hides_credentials_after_a_line_break_in_an_argument() {
        assert_eq!(
            command_echo(&args(&["tag", "-a", "v1", "-m", "Release 1.2\nhttps://ghp_TOKEN@github.com/o/r"])),
            r#"$ git tag -a v1 -m "Release 1.2\nhttps://***@github.com/o/r""#
        );
        assert_eq!(command_echo(&args(&["x", "a\r\nhttps://u:tok@h/x"])), r#"$ git x "a\r\nhttps://***@h/x""#);
        assert_eq!(command_echo(&args(&["x", "https://h\nhttps://u:tok@h2/x"])), r#"$ git x "https://h\nhttps://***@h2/x""#);
    }

    #[test]
    fn command_echo_of_plain_arguments_is_unchanged() {
        for (a, echo) in [
            (args(&["push", "https://u:TOKEN@h/o/r.git", "main"]), "$ git push https://***@h/o/r.git main"),
            (args(&["stash", "pop", "--index", "stash@{0}"]), "$ git stash pop --index stash@{0}"),
            (args(&["worktree", "remove", "C:/x/y"]), "$ git worktree remove C:/x/y"),
            (args(&["clone", "ssh://git@h/x"]), "$ git clone ssh://git@h/x"),
        ] {
            assert_eq!(command_echo(&a), echo);
        }
    }
}
