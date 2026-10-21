//! Reading a job's output (`$ git <args>` echoes followed by git's combined stdout and stderr)
//! for the activity panel: splitting it into lines as a terminal would show them, telling the
//! kinds of line apart and picking the one that best sums up what happened.

/// The longest summary [`last_message`] returns, in characters (then "...").
const SUMMARY_CHARS: usize = 200;

/// The lines of `text` as a terminal would leave them: for each line only the text after its
/// last carriage return (progress redraws itself with '\r'), and a trailing '\r' (CRLF output)
/// dropped first.
pub fn output_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|line| {
        let line = line.strip_suffix('\r').unwrap_or(line);
        match line.rfind('\r') {
            Some(i) => &line[i + 1..],
            None => line,
        }
    })
}

/// What kind of line a job's output line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputLine {
    /// Glowfly's echo of the command it ran (`$ git push`).
    Command,
    /// git's `fatal:` and `error:` lines.
    Error,
    /// git's `warning:` lines.
    Warning,
    /// git's advice (`hint:`), which config can turn off.
    Hint,
    /// Text relayed from the server (`remote:`).
    Remote,
    Plain,
}

/// The kind of `line` (one line of [`output_lines`]).
pub fn classify_line(line: &str) -> OutputLine {
    if line.starts_with("$ ") {
        OutputLine::Command
    } else if line.starts_with("fatal:") || line.starts_with("error:") {
        OutputLine::Error
    } else if line.starts_with("warning:") {
        OutputLine::Warning
    } else if line.starts_with("hint:") {
        OutputLine::Hint
    } else if line.starts_with("remote:") {
        OutputLine::Remote
    } else {
        OutputLine::Plain
    }
}

/// The last meaningful line of `output`: not an echo, not advice and not blank, trimmed and at
/// most `SUMMARY_CHARS` characters (then "..."). Empty when there is none.
pub fn last_message(output: &str) -> String {
    let line = output_lines(output)
        .map(str::trim)
        .filter(|l| !l.is_empty() && !matches!(classify_line(l), OutputLine::Command | OutputLine::Hint))
        .last()
        .unwrap_or_default();
    match line.char_indices().nth(SUMMARY_CHARS) {
        Some((end, _)) => format!("{}...", &line[..end]),
        None => line.to_owned(),
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
