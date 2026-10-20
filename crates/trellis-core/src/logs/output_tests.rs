use super::*;

fn lines(text: &str) -> Vec<&str> {
    output_lines(text).collect()
}

#[test]
fn output_lines_keep_what_a_terminal_shows() {
    assert_eq!(lines("one\r\ntwo\r\n"), ["one", "two", ""]);
    assert_eq!(lines("remote: Counting 10%\rremote: Counting 100%"), ["remote: Counting 100%"]);
    assert_eq!(lines("a\rb\r\nc"), ["b", "c"]);
    assert_eq!(lines(""), [""]);
    assert_eq!(lines("\r"), [""]);
}

#[test]
fn classify_line_tells_kinds_apart() {
    assert_eq!(classify_line("$ git push"), OutputLine::Command);
    assert_eq!(classify_line("fatal: x"), OutputLine::Error);
    assert_eq!(classify_line("error: x"), OutputLine::Error);
    assert_eq!(classify_line("warning: x"), OutputLine::Warning);
    assert_eq!(classify_line("hint: x"), OutputLine::Hint);
    assert_eq!(classify_line("remote: x"), OutputLine::Remote);
    assert_eq!(classify_line("Everything up-to-date"), OutputLine::Plain);
    assert_eq!(classify_line("$git"), OutputLine::Plain, "an echo has a space after the $");
    assert_eq!(classify_line(""), OutputLine::Plain);
}

#[test]
fn last_message_skips_echoes_hints_and_blanks() {
    assert_eq!(last_message("$ git pull\nAlready up to date.\nhint: try this\n\n"), "Already up to date.");
    assert_eq!(last_message("$ git fetch\n"), "");
    assert_eq!(last_message(""), "");
    assert_eq!(last_message("  remote: done  \r\n"), "remote: done");
    let long = "x".repeat(250);
    let cut = last_message(&format!("$ git push\n{long}"));
    assert_eq!(cut, format!("{}...", "x".repeat(200)));
    let exact = "y".repeat(200);
    assert_eq!(last_message(&exact), exact);
}
