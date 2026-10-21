use super::super::{Confirm, Kind, MAX_COMMANDS, Place, ProblemKind};
use super::*;

/// The problems of `text`, as (line, kind).
fn problems(text: &str) -> Vec<(Option<usize>, ProblemKind)> {
    parse(text).1.into_iter().map(|p| (p.line, p.kind)).collect()
}

/// The one problem of a file with one command whose fields are `body`, and that no command
/// loaded.
fn only_problem(body: &str) -> (Option<usize>, ProblemKind) {
    let text = format!("[[command]]\n{body}\n");
    let (commands, problems) = parse(&text);
    assert!(commands.is_empty(), "{commands:?}");
    let [p] = <[_; 1]>::try_from(problems).unwrap_or_else(|p| panic!("one problem expected: {p:?}"));
    (p.line, p.kind)
}

const TWO_OF_EACH: &str = r#"# My commands

[[command]]
name = "Log"
kind = "git"
template = "log --oneline {sha}"

[[command]]
name = "Status"
kind = "git"
template = "git status"
show = ["palette", "commit", "palette"]

[[command]]
name = "Format"
kind = "shell"
template = "cargo fmt -- {files}"
show = ["file"]
key = " Mod+Alt+F "

[[command]]
name = "Hello"
kind = "shell"
template = "echo hello"
"#;

#[test]
fn a_valid_file_parses_with_defaults() {
    let (commands, problems) = parse(TWO_OF_EACH);
    assert!(problems.is_empty(), "{problems:?}");
    let names: Vec<_> = commands.iter().map(|c| (c.name.as_str(), c.kind, c.line)).collect();
    assert_eq!(names, [("Log", Kind::Git, 3), ("Status", Kind::Git, 8), ("Format", Kind::Shell, 14), ("Hello", Kind::Shell, 21)]);
    let log = &commands[0];
    assert_eq!((log.show.as_slice(), &log.confirm, &log.key), ([Place::Palette].as_slice(), &Confirm::No, &None));
    assert!(log.needs().sha && !log.needs().branch);
    assert_eq!(commands[1].show, [Place::Palette, Place::Commit], "listed once each");
    assert_eq!((commands[2].key.as_deref(), commands[2].key_line), (Some("Mod+Alt+F"), 19));
    assert!(commands[2].shows_in(Place::File) && !commands[2].shows_in(Place::Palette));
    assert_eq!(parse(""), (Vec::new(), Vec::new()), "an empty file has nothing");
}

#[test]
fn a_syntax_error_loads_nothing_and_says_where() {
    let text = "[[command]]\nname = \"A\"\nkind = \"git\"\ntemplate = \"log\n";
    let (commands, problems) = parse(text);
    assert!(commands.is_empty());
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].line, Some(4));
    assert!(matches!(&problems[0].kind, ProblemKind::Syntax(m) if !m.contains("template")), "{problems:?}");
    assert_eq!(parse(&text.replace('\n', "\r\n")).1[0].line, Some(4), "CRLF counts each line once");
    let bom = format!("\u{feff}{TWO_OF_EACH}");
    assert_eq!(parse(&bom).0.len(), 4, "a byte order mark is skipped");
}

#[test]
fn unknown_fields_wrong_types_and_a_missing_kind_are_errors_of_the_file() {
    let unknown = "[[command]]\nname = \"A\"\nkind = \"git\"\ntemplate = \"log\"\n\ntemplat = \"x\"\n";
    let p = problems(unknown);
    assert!(matches!(p.as_slice(), [(Some(6), ProblemKind::Syntax(_))]), "{p:?}");
    let wrong = "[[command]]\nname = \"A\"\nkind = \"git\"\ntemplate = \"log\"\nshow = \"palette\"\n";
    let p = problems(wrong);
    assert!(matches!(p.as_slice(), [(Some(5), ProblemKind::Syntax(_))]), "{p:?}");
    let no_kind = "[[command]]\nname = \"A\"\ntemplate = \"log\"\n";
    let p = problems(no_kind);
    assert!(matches!(p.as_slice(), [(Some(_), ProblemKind::Syntax(m))] if m.contains("kind")), "{p:?}");
    let top = "verbose = true\n";
    assert!(matches!(problems(top).as_slice(), [(Some(1), ProblemKind::Syntax(_))]));
    assert!(parse(no_kind).0.is_empty());
}

#[test]
fn a_problem_drops_only_its_command() {
    let text = "[[command]]\nname = \"Good\"\nkind = \"git\"\ntemplate = \"log\"\n\n[[command]]\nname = \"Bad\"\nkind = \"git\"\ntemplate = \"log {brnach}\"\n";
    let (commands, problems) = parse(text);
    assert_eq!(commands.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Good"]);
    assert_eq!(problems, [Problem { line: Some(9), kind: ProblemKind::UnknownPlaceholder("{brnach}".into()) }]);
}

#[test]
fn per_command_problems_have_their_field_lines() {
    let git = |t: &str| format!("name = \"A\"\nkind = \"git\"\ntemplate = '{t}'");
    let shell = |t: &str| format!("name = \"A\"\nkind = \"shell\"\ntemplate = '{t}'");
    assert_eq!(only_problem("name = \"  \"\nkind = \"git\"\ntemplate = \"log\""), (Some(2), ProblemKind::EmptyName));
    assert_eq!(only_problem("kind = \"git\"\ntemplate = \"log\""), (Some(1), ProblemKind::EmptyName));
    let long = format!("name = \"{}\"\nkind = \"git\"\ntemplate = \"log\"", "x".repeat(81));
    assert_eq!(only_problem(&long), (Some(2), ProblemKind::LongName));
    assert_eq!(only_problem("name = \"A\"\nkind = \"git\"\ntemplate = \"  \""), (Some(4), ProblemKind::EmptyTemplate));
    assert_eq!(only_problem("name = \"A\"\nkind = \"git\""), (Some(1), ProblemKind::EmptyTemplate));
    assert_eq!(only_problem(&format!("{}\nshow = []", git("log"))), (Some(5), ProblemKind::NoPlaces));
    assert_eq!(only_problem(&git("log {brnach}")), (Some(4), ProblemKind::UnknownPlaceholder("{brnach}".into())));
    assert_eq!(only_problem(&git("log --author=x{files}")), (Some(4), ProblemKind::FilesNotAlone));
    assert_eq!(only_problem(&git("log \"unclosed")), (Some(4), ProblemKind::UnmatchedQuote));
    assert_eq!(only_problem(&shell("echo \"{branch}\"")), (Some(4), ProblemKind::QuotedPlaceholder("{branch}".into())));
    let single = "name = \"A\"
kind = \"shell\"
template = \"echo '{sha}'\"";
    assert_eq!(only_problem(single), (Some(4), ProblemKind::QuotedPlaceholder("{sha}".into())));
    assert_eq!(only_problem(&shell("echo \\{sha}")), (Some(4), ProblemKind::QuotedPlaceholder("{sha}".into())));
    assert_eq!(only_problem(&shell("echo \"open")), (Some(4), ProblemKind::UnmatchedQuote));
    let dup =
        "[[command]]\nname = \"A\"\nkind = \"git\"\ntemplate = \"log\"\n[[command]]\nname = \"A\"\nkind = \"git\"\ntemplate = \"status\"\n";
    let (commands, problems) = parse(dup);
    assert_eq!(commands.len(), 1);
    assert_eq!(problems, [Problem { line: Some(6), kind: ProblemKind::DuplicateName("A".into()) }]);
}

#[test]
fn git_and_shell_syntax_after_at_and_dollar_is_not_a_placeholder() {
    for t in ["log @{u}..HEAD {branch}", "reflog HEAD@{1} {branch}", "stash show stash@{0} {branch}"] {
        let text = format!("[[command]]\nname = \"A\"\nkind = \"git\"\ntemplate = \"{t}\"\n");
        let (commands, problems) = parse(&text);
        assert!(problems.is_empty(), "{t}: {problems:?}");
        let n = commands[0].needs();
        assert!(n.branch && !n.sha && !n.files && !n.repo, "{t}");
    }
    let text = "[[command]]\nname = \"A\"\nkind = \"shell\"\ntemplate = 'echo ${HOME} {branch}'\n";
    let (commands, problems) = parse(text);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(commands[0].needs(), super::super::Needs { branch: true, ..Default::default() });
}

#[test]
fn confirm_limits_and_names() {
    let with = |extra: &str| {
        let text = format!("[[command]]\nname = \"A\\u0007 b\"\nkind = \"git\"\ntemplate = \"log\"\n{extra}\n");
        parse(&text).0.remove(0)
    };
    assert_eq!(with("confirm = true").confirm, Confirm::Default);
    assert_eq!(with("confirm = false").confirm, Confirm::No);
    assert_eq!(with("confirm = \" Reset {branch}? \"").confirm, Confirm::Text("Reset {branch}?".into()));
    assert_eq!(with("").name, "A b", "control characters are stripped");
    let many: String = (0..=MAX_COMMANDS).map(|i| format!("[[command]]\nname = \"C{i}\"\nkind = \"git\"\ntemplate = \"log\"\n")).collect();
    let (commands, problems) = parse(&many);
    assert_eq!(commands.len(), MAX_COMMANDS);
    assert_eq!(problems, [Problem { line: Some(MAX_COMMANDS * 4 + 1), kind: ProblemKind::TooMany }]);
}

#[test]
fn lines_count_newlines() {
    assert_eq!(line_of("a\nb\r\nc", 0), 1);
    assert_eq!(line_of("a\nb\r\nc", 2), 2);
    assert_eq!(line_of("a\nb\r\nc", 5), 3);
    assert_eq!(line_of("a", 99), 1);
}
