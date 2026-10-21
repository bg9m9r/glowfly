//! Pure tests of the todo list: building it, checking it, quoting, and installing it over git's.

use super::*;
use git2::Oid;
use std::path::Path;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn item(n: u8, action: RebaseAction) -> TodoItem {
    TodoItem {
        oid: oid(n),
        action,
        summary: format!("commit {n}"),
        message: format!("commit {n}\n\nbody {n}\n"),
        new_message: None,
        combined_message: None,
        author: "Test".into(),
        time: 1_700_000_000,
    }
}

fn reword(n: u8, message: &str) -> TodoItem {
    TodoItem { new_message: Some(message.into()), ..item(n, RebaseAction::Reword) }
}

fn steps(items: &[TodoItem]) -> Vec<TodoStep> {
    items.iter().cloned().map(TodoStep::Commit).collect()
}

fn c(n: u8, action: RebaseAction) -> TodoStep {
    TodoStep::Commit(item(n, action))
}

fn exec(cmd: &str) -> TodoStep {
    TodoStep::Exec(cmd.into())
}

/// The message folder in this OS's own spelling, and how it appears in the todo for `sh`
/// (backslashes become slashes only on Windows; on Unix a backslash is part of a name).
#[cfg(windows)]
const MSG_DIR: &str = "C:\\repo\\.git\\glowfly-rebase";
#[cfg(windows)]
const QUOTED_DIR: &str = "C:/repo/.git/glowfly-rebase";
#[cfg(not(windows))]
const MSG_DIR: &str = "/repo/.git/glowfly-rebase";
#[cfg(not(windows))]
const QUOTED_DIR: &str = "/repo/.git/glowfly-rebase";

fn base_line(n: u8) -> String {
    format!("exec git rev-parse HEAD >'{QUOTED_DIR}/reword-{}.base'", oid(n))
}

fn exec_line(n: u8) -> String {
    let (base, file) = (format!("'{QUOTED_DIR}/reword-{0}.base'", oid(n)), format!("'{QUOTED_DIR}/reword-{0}.txt'", oid(n)));
    format!(
        r#"exec h=$(git rev-parse -q --verify HEAD); p=$(git rev-parse -q --verify HEAD^); b=$(cat {base}); if [ "$h" != "$b" ] && {{ [ "$p" = "$b" ] || [ -z "$p" ]; }}; then git commit --amend --allow-empty --no-verify --cleanup=whitespace -F {file}; fi"#
    )
}

#[test]
fn build_writes_one_line_per_row_in_order() {
    let mut items: Vec<TodoItem> = RebaseAction::ALL.iter().enumerate().map(|(i, a)| item(i as u8 + 1, *a)).collect();
    items[0].summary = "two\nlines".into();
    let (text, files) = build(&steps(&items), Path::new(MSG_DIR), false);
    let expected: Vec<String> = ["pick", "pick", "squash", "fixup", "drop", "edit"]
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let summary = if i == 0 { "two lines".to_owned() } else { format!("commit {}", i + 1) };
            format!("{k} {} {summary}", oid(i as u8 + 1))
        })
        .collect();
    assert_eq!(text, expected.join("\n") + "\n", "a reword without a new message is a plain pick");
    assert!(!text.contains('\r'));
    assert!(files.is_empty());
}

#[test]
fn a_reword_amends_from_a_message_file() {
    let items = [item(1, RebaseAction::Pick), reword(2, "New title\n\nNew body\n\n"), item(3, RebaseAction::Pick)];
    let (text, files) = build(&steps(&items), Path::new(MSG_DIR), false);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[1], base_line(2), "HEAD is recorded before the reworded pick");
    assert_eq!(lines[2], format!("pick {} commit 2", oid(2)));
    assert_eq!(lines[3], exec_line(2));
    assert!(lines[4].starts_with("pick "));
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].0, Path::new(MSG_DIR).join(format!("reword-{}.txt", oid(2))));
    assert_eq!(files[0].1, "New title\n\nNew body\n");

    // An unchanged message (ignoring surrounding whitespace) is no reword at all.
    let same = reword(2, "  commit 2\n\nbody 2");
    let (text, files) = build(&steps(&[same]), Path::new(MSG_DIR), false);
    assert_eq!(text, format!("pick {} commit 2\n", oid(2)));
    assert!(files.is_empty());
}

#[test]
fn a_reword_exec_follows_its_whole_group() {
    let items = [
        reword(1, "Combined"),
        item(2, RebaseAction::Squash),
        item(3, RebaseAction::Drop),
        item(4, RebaseAction::Fixup),
        item(5, RebaseAction::Pick),
    ];
    let (text, _) = build(&steps(&items), Path::new(MSG_DIR), false);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], base_line(1));
    assert!(lines[4].starts_with("fixup "), "{text}");
    assert_eq!(lines[5], squashed_line(1));
    assert!(lines[6].starts_with("pick "));
}

fn squashed_line(n: u8) -> String {
    exec_line(n).replace(&format!("/reword-{}.txt", oid(n)), &format!("/squashed-{}.txt", oid(n)))
}

#[test]
fn a_reworded_group_head_gets_the_message_its_editor_shows() {
    use RebaseAction::*;
    let list = [TodoStep::Commit(reword(1, "Typed\n")), c(2, Squash), c(3, Fixup), c(4, Pick)];
    let (text, files) = build(&list, Path::new(MSG_DIR), false);
    assert_eq!(text.lines().nth(4), Some(squashed_line(1).as_str()), "{text}");
    let dir = Path::new(MSG_DIR);
    let want = [
        (dir.join(format!("reword-{}.txt", oid(1))), "Typed\n".to_owned()),
        (dir.join(format!("squashed-{}.txt", oid(1))), default_combined(&list, 0) + "\n"),
    ];
    assert_eq!(files, want);
    assert_eq!(files[1].1, "Typed\n\ncommit 2\n\nbody 2\n", "the squash's message is kept");

    // With only fixups, the typed message is the whole message.
    let fixups = [TodoStep::Commit(reword(1, "Typed")), c(2, Fixup)];
    let (text, files) = build(&fixups, Path::new(MSG_DIR), false);
    assert_eq!(text.lines().nth(3), Some(exec_line(1).as_str()), "{text}");
    assert_eq!(files, [(dir.join(format!("reword-{}.txt", oid(1))), default_combined(&fixups, 0) + "\n")]);
}

#[test]
fn problems_with_the_list() {
    use RebaseAction::*;
    let list = |actions: &[RebaseAction]| actions.iter().enumerate().map(|(i, a)| c(i as u8, *a)).collect::<Vec<_>>();
    assert_eq!(problem(&list(&[Squash, Pick])).as_deref(), Some(FIRST_COMBINES));
    assert_eq!(problem(&list(&[Fixup])).as_deref(), Some(FIRST_COMBINES));
    assert_eq!(problem(&list(&[Drop, Drop, Squash])).as_deref(), Some(FIRST_COMBINES));
    assert_eq!(problem(&list(&[Drop, Pick, Squash, Fixup])), None);
    assert_eq!(problem(&steps(&[reword(1, "")])).as_deref(), Some(EMPTY_REWORD));
    assert_eq!(problem(&steps(&[reword(1, " \n\t ")])).as_deref(), Some(EMPTY_REWORD));
    assert_eq!(problem(&[c(1, Reword)]), None, "a reword without a typed message keeps the original");
}

#[test]
fn unchanged_lists() {
    let original = vec![c(1, RebaseAction::Pick), c(2, RebaseAction::Pick)];
    assert!(is_unchanged(&original, &original));
    assert!(!is_unchanged(&[original[1].clone(), original[0].clone()], &original));
    let mut edited = original.clone();
    edited[1].commit_mut().unwrap().action = RebaseAction::Reword;
    assert!(is_unchanged(&edited, &original), "a reword keeping its message changes nothing");
    edited[1].commit_mut().unwrap().new_message = Some("Other".into());
    assert!(!is_unchanged(&edited, &original));
    edited[1] = c(2, RebaseAction::Drop);
    assert!(!is_unchanged(&edited, &original));
    let mut with_step = original.clone();
    with_step.push(TodoStep::Break);
    assert!(!is_unchanged(&with_step, &original), "a break step is a change");
    with_step[2] = exec("true");
    assert!(!is_unchanged(&with_step, &original), "an exec step is a change");
    let mut combined = original.clone();
    combined[0].commit_mut().unwrap().combined_message = Some("Both".into());
    assert!(!is_unchanged(&combined, &original), "a typed combined message is a change");
}

#[test]
fn quoting_and_editor_commands() {
    assert_eq!(shell_quote_for("C:\\Program Files\\Glowfly\\glowfly.exe", true), "'C:/Program Files/Glowfly/glowfly.exe'");
    // On Unix a backslash is part of a name: `/tmp/a\b` is not `/tmp/a/b`.
    assert_eq!(shell_quote_for("/tmp/a\\b/todo", false), "'/tmp/a\\b/todo'");
    assert_eq!(shell_quote("it's"), "'it'\\''s'");
    assert_eq!(shell_quote("/repo/x"), "'/repo/x'");
    if cfg!(windows) {
        let prepared = Path::new("C:\\repo\\.git\\glowfly-rebase\\todo");
        let exe = SequenceEditor::Helper("C:\\Apps\\glowfly.exe".into());
        assert_eq!(editor_command(&exe, prepared), "'C:/Apps/glowfly.exe' --write-todo 'C:/repo/.git/glowfly-rebase/todo'");
        assert_eq!(editor_command(&SequenceEditor::Copy, prepared), "cp 'C:/repo/.git/glowfly-rebase/todo'");
    }
    let prepared = Path::new("/repo/.git/glowfly-rebase/todo");
    let exe = SequenceEditor::Helper("/opt/glowfly/glowfly".into());
    assert_eq!(editor_command(&exe, prepared), "'/opt/glowfly/glowfly' --write-todo '/repo/.git/glowfly-rebase/todo'");
    assert_eq!(editor_command(&SequenceEditor::Shell("false".into()), prepared), "false");
}

#[test]
fn listed_oids_reads_git_todo_text() {
    let text = "\
pick 1a2b3c4 First
# a comment line
p 2b3c4d5 Second

s 3c4d5e6 Third
f 4d5e6f7 Fourth
fixup -C 5e6f7a8 Fifth
exec make test
label onto
update-ref refs/heads/topic
break
d 6f7a8b9 Sixth
";
    assert_eq!(listed_oids(text), ["1a2b3c4", "2b3c4d5", "3c4d5e6", "4d5e6f7", "5e6f7a8", "6f7a8b9"]);
}

#[test]
fn install_todo_checks_the_commits() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-test-install-todo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (prepared, git_todo) = (dir.join("todo"), dir.join("git-rebase-todo"));
    let short = |n: u8| oid(n).to_string()[..7].to_owned();
    let git_text = format!("pick {} commit 1\npick {} commit 2\n\n# Rebase onto ...\n", short(1), short(2));
    let (ours, _) = build(&[c(2, RebaseAction::Pick), c(1, RebaseAction::Drop)], Path::new(MSG_DIR), false);
    std::fs::write(&prepared, &ours).unwrap();

    std::fs::write(&git_todo, &git_text).unwrap();
    install_todo(&prepared, &git_todo).unwrap();
    assert_eq!(std::fs::read_to_string(&git_todo).unwrap(), ours);

    // An extra commit in git's list, or one missing, leaves git's todo untouched.
    for text in [format!("{git_text}pick {} commit 3\n", short(3)), format!("pick {} commit 1\n", short(1))] {
        std::fs::write(&git_todo, &text).unwrap();
        let e = install_todo(&prepared, &git_todo).unwrap_err();
        assert_eq!(e.to_string(), CHANGED);
        assert_eq!(std::fs::read_to_string(&git_todo).unwrap(), text);
    }
    // The same count, but a different commit.
    let other = format!("pick {} commit 1\npick {} commit 3\n", short(1), short(3));
    std::fs::write(&git_todo, &other).unwrap();
    assert!(install_todo(&prepared, &git_todo).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exec_and_break_lines_in_order() {
    use RebaseAction::*;
    let list = [c(1, Pick), exec("cargo test"), c(2, Pick), TodoStep::Break, c(3, Pick)];
    let (text, files) = build(&list, Path::new(MSG_DIR), false);
    let want = format!("pick {} commit 1\nexec cargo test\npick {} commit 2\nbreak\npick {} commit 3\n", oid(1), oid(2), oid(3));
    assert_eq!(text, want);
    assert!(files.is_empty());
}

#[test]
fn a_reword_amends_before_a_following_exec() {
    let list = [TodoStep::Commit(reword(1, "New")), exec("make check"), c(2, RebaseAction::Pick)];
    let (text, _) = build(&list, Path::new(MSG_DIR), false);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], base_line(1));
    assert!(lines[1].starts_with("pick "));
    assert_eq!(lines[2], exec_line(1), "the amend comes before the exec step");
    assert_eq!(lines[3], "exec make check");
}

#[test]
fn a_combined_message_amends_after_the_group() {
    use RebaseAction::*;
    let mut head = item(1, Pick);
    head.combined_message = Some("All in one\n\n".into());
    let list = [TodoStep::Commit(head), c(2, Squash), c(3, Fixup), c(4, Pick)];
    let (text, files) = build(&list, Path::new(MSG_DIR), false);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], base_line(1));
    assert!(lines[3].starts_with("fixup "), "{text}");
    let file = format!("'{QUOTED_DIR}/combined-{}.txt'", oid(1));
    assert_eq!(lines[4], exec_line(1).replace(&format!("'{QUOTED_DIR}/reword-{}.txt'", oid(1)), &file));
    assert!(lines[5].starts_with("pick "));
    assert_eq!(files, [(Path::new(MSG_DIR).join(format!("combined-{}.txt", oid(1))), "All in one\n".to_owned())]);

    // Without a group to head, a combined message is ignored.
    let mut alone = item(1, Pick);
    alone.combined_message = Some("Lonely".into());
    let (text, files) = build(&[TodoStep::Commit(alone)], Path::new(MSG_DIR), false);
    assert_eq!(text, format!("pick {} commit 1\n", oid(1)));
    assert!(files.is_empty());
}

#[test]
fn keep_rewords_writes_untyped_rewords() {
    let list = [c(1, RebaseAction::Reword)];
    assert_eq!(build(&list, Path::new(MSG_DIR), true).0, format!("reword {} commit 1\n", oid(1)));
    assert_eq!(build(&list, Path::new(MSG_DIR), false).0, format!("pick {} commit 1\n", oid(1)));
}

#[test]
fn group_end_and_default_combined() {
    use RebaseAction::*;
    let list = [c(1, Pick), c(2, Squash), c(3, Drop), c(4, Fixup), c(5, Squash), TodoStep::Break, c(6, Squash)];
    assert_eq!(group_end(&list, 0), Some(4), "drops are skipped, a break ends the group");
    assert_eq!(group_end(&list, 1), None, "a squash heads nothing");
    assert_eq!(group_end(&[c(1, Pick), c(2, Pick)], 0), None);
    assert_eq!(group_end(&[c(1, Pick), exec("x"), c(2, Squash)], 0), None, "an exec step ends the group");
    assert_eq!(default_combined(&list, 0), "commit 1\n\nbody 1\n\ncommit 2\n\nbody 2\n\ncommit 5\n\nbody 5");
    let mut reworded = list.to_vec();
    reworded[0] = TodoStep::Commit(reword(1, "Typed\n"));
    assert!(default_combined(&reworded, 0).starts_with("Typed\n\ncommit 2"), "{}", default_combined(&reworded, 0));
    assert_eq!(default_combined(&[c(1, Pick)], 0), "commit 1\n\nbody 1");
}

#[test]
fn problems_with_steps() {
    use RebaseAction::*;
    assert_eq!(problem(&[exec("x"), c(1, Squash)]).as_deref(), Some(FIRST_COMBINES));
    assert_eq!(problem_with(&[c(1, Squash), c(2, Pick)], true), None, "it melds into the commit already done");
    assert_eq!(problem_with(&[TodoStep::Break, c(1, Fixup)], true).as_deref(), Some(COMBINE_AFTER_STEP));
    assert_eq!(problem(&[c(1, Pick), exec("x"), c(2, Squash)]).as_deref(), Some(COMBINE_AFTER_STEP));
    assert_eq!(problem(&[c(1, Pick), TodoStep::Break, c(3, Drop), c(2, Fixup)]).as_deref(), Some(COMBINE_AFTER_STEP));
    assert_eq!(problem(&[c(1, Pick), c(2, Squash), exec("x")]), None);
    let mut head = item(1, Pick);
    head.combined_message = Some("  \n".into());
    assert_eq!(problem(&[TodoStep::Commit(head.clone()), c(2, Squash)]).as_deref(), Some(EMPTY_COMBINED));
    assert_eq!(problem(&[TodoStep::Commit(head)]), None, "no group, so the message isn't used");
    assert_eq!(problem(&[c(1, Pick), exec("  ")]).as_deref(), Some(EMPTY_EXEC));
    assert_eq!(problem(&[c(1, Pick), exec("a\nb")]).as_deref(), Some(EXEC_ONE_LINE));
    assert_eq!(problem(&[c(1, Pick), exec("a\rb")]).as_deref(), Some(EXEC_ONE_LINE));
}

#[test]
fn own_lines_are_recognised() {
    use super::todo::{MessageFile, OwnLine, own_line};
    let mut head = item(1, RebaseAction::Pick);
    head.combined_message = Some("Both".into());
    let list = [
        TodoStep::Commit(reword(2, "New")),
        TodoStep::Commit(head),
        c(3, RebaseAction::Squash),
        TodoStep::Commit(reword(4, "Four")),
        c(5, RebaseAction::Squash),
    ];
    let (text, _) = build(&list, Path::new(MSG_DIR), false);
    // The lines as a Windows folder (backslashes turned to slashes) and a Unix one quote.
    for (dir, windows) in [("C:\\repo\\.git\\glowfly-rebase", true), ("/home/u/repo/.git/glowfly-rebase", false)] {
        let quoted = shell_quote_for(dir, windows);
        let text = text.replace(QUOTED_DIR, quoted.trim_matches('\''));
        let found: Vec<Option<OwnLine>> = text.lines().map(own_line).collect();
        let want = [
            Some(OwnLine::Base(oid(2))),
            None,
            Some(OwnLine::Amend { oid: oid(2), file: MessageFile::Reword }),
            Some(OwnLine::Base(oid(1))),
            None,
            None,
            Some(OwnLine::Amend { oid: oid(1), file: MessageFile::Combined }),
            Some(OwnLine::Base(oid(4))),
            None,
            None,
            Some(OwnLine::Amend { oid: oid(4), file: MessageFile::Squashed }),
        ];
        assert_eq!(found, want, "{text}");
    }
    assert_eq!(own_line("exec make test"), None);
    assert_eq!(own_line("exec cat glowfly-rebase/reword-1234.base"), None);
}
