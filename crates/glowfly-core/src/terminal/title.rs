//! The window title a program sets (OSC 0 and OSC 2), kept for the terminal tab's name, and
//! what of it is worth showing: a shell's title that only names the shell itself is not.

/// The longest title kept, in chars.
const MAX_TITLE: usize = 256;

/// What the parser reports besides the screen: the last window title, if any.
#[derive(Default)]
pub(super) struct ScreenEvents {
    pub(super) title: Option<String>,
}

impl vt100::Callbacks for ScreenEvents {
    /// OSC 0 and OSC 2. OSC 1 (the icon name) never reaches here.
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = clean(title);
    }

    /// A title with a ';' in it arrives split into more parameters; join them back.
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        if let [b"0" | b"2", rest @ ..] = params
            && rest.len() > 1
        {
            self.title = clean(&rest.join(&b';'));
        }
    }
}

/// `bytes` as text without control characters, trimmed and at most `MAX_TITLE` chars; None
/// when nothing is left.
fn clean(bytes: &[u8]) -> Option<String> {
    let text: String = String::from_utf8_lossy(bytes).chars().filter(|c| !c.is_control()).collect();
    let text: String = text.trim().chars().take(MAX_TITLE).collect();
    let text = text.trim_end();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The last path segment of `s` (split on both '/' and '\', whatever the OS), without one
/// trailing ".exe" in any case.
fn path_stem(s: &str) -> &str {
    let last = s.rsplit(['/', '\\']).next().unwrap_or(s);
    let n = last.len();
    if n > 4 && last.is_char_boundary(n - 4) && last[n - 4..].eq_ignore_ascii_case(".exe") { &last[..n - 4] } else { last }
}

/// What a tab shows for the window title `title` of a shell whose short name is `program`:
/// None when the title only names the shell (its own path, as Windows' consoles start with);
/// the rest of "<shell path> - <command>" (cmd while it runs a command); otherwise the title.
/// An empty `program` means no shell is known, and the trimmed title is shown.
pub fn command_from_title(title: &str, program: &str) -> Option<String> {
    let raw = title;
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    if program.is_empty() {
        return Some(title.to_owned());
    }
    let is_shell = |s: &str| path_stem(s.trim()).eq_ignore_ascii_case(program);
    if is_shell(title) {
        return None;
    }
    // Split before trimming, so "<shell path> - " with nothing after still splits.
    if let Some((head, rest)) = raw.trim_start().split_once(" - ")
        && is_shell(head)
    {
        let rest = rest.trim();
        return (!rest.is_empty()).then(|| rest.to_owned());
    }
    // A cleaned title has lost the space after the dash.
    if title.strip_suffix(" -").is_some_and(is_shell) {
        return None;
    }
    Some(title.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Windows path separator, so no drive path is written out.
    const SEP: char = '\\';

    fn win_path(parts: &[&str]) -> String {
        let mut s = String::from("C:");
        for p in parts {
            s.push(SEP);
            s.push_str(p);
        }
        s
    }

    #[test]
    fn the_shells_own_path_is_not_a_command() {
        let pwsh = win_path(&["Program Files", "PowerShell", "7", "pwsh.exe"]);
        assert_eq!(command_from_title(&pwsh, "pwsh"), None);
        assert_eq!(command_from_title("PWSH.EXE", "pwsh"), None, "case-insensitive");
        assert_eq!(command_from_title("/usr/bin/bash", "bash"), None);
    }

    #[test]
    fn cmd_shows_the_command_after_its_path() {
        let cmd = win_path(&["Windows", "system32", "cmd.exe"]);
        assert_eq!(command_from_title(&format!("{cmd} - cargo  test"), "cmd"), Some("cargo  test".into()));
        assert_eq!(command_from_title(&format!("{cmd} - "), "cmd"), None);
        assert_eq!(command_from_title(&format!("{cmd} -"), "cmd"), None, "as a cleaned title has it");
    }

    #[test]
    fn other_titles_are_shown_as_is() {
        assert_eq!(command_from_title("vim notes.md", "bash"), Some("vim notes.md".into()));
        assert_eq!(command_from_title("", "bash"), None);
        assert_eq!(command_from_title("   ", "bash"), None);
        assert_eq!(command_from_title("  htop ", ""), Some("htop".into()), "no shell known");
    }

    #[test]
    fn clean_drops_controls_and_caps_the_length() {
        assert_eq!(clean(b" a\x07b\x1bc "), Some("abc".into()));
        assert_eq!(clean(b"  \t "), None);
        assert_eq!(clean("x".repeat(300).as_bytes()).map(|s| s.chars().count()), Some(MAX_TITLE));
        assert_eq!(path_stem("a/b\\c.EXE"), "c");
        assert_eq!(path_stem(".exe"), ".exe");
    }
}
