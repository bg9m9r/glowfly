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
        author: "Test".into(),
        time: 1_700_000_000,
    }
}

fn reword(n: u8, message: &str) -> TodoItem {
    TodoItem { new_message: Some(message.into()), ..item(n, RebaseAction::Reword) }
}

const MSG_DIR: &str = "C:\\repo\\.git\\trellis-rebase";

fn base_line(n: u8) -> String {
    format!("exec git rev-parse HEAD >'C:/repo/.git/trellis-rebase/reword-{}.base'", oid(n))
}

fn exec_line(n: u8) -> String {
    let (base, file) =
        (format!("'C:/repo/.git/trellis-rebase/reword-{0}.base'", oid(n)), format!("'C:/repo/.git/trellis-rebase/reword-{0}.txt'", oid(n)));
    format!(
        r#"exec h=$(git rev-parse -q --verify HEAD); p=$(git rev-parse -q --verify HEAD^); b=$(cat {base}); if [ "$h" != "$b" ] && {{ [ "$p" = "$b" ] || [ -z "$p" ]; }}; then git commit --amend --allow-empty --no-verify --cleanup=whitespace -F {file}; fi"#
    )
}

#[test]
fn build_writes_one_line_per_row_in_order() {
    let mut items: Vec<TodoItem> = RebaseAction::ALL.iter().enumerate().map(|(i, a)| item(i as u8 + 1, *a)).collect();
    items[0].summary = "two\nlines".into();
    let (text, files) = build(&items, Path::new(MSG_DIR));
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
    let (text, files) = build(&items, Path::new(MSG_DIR));
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
    let (text, files) = build(&[same], Path::new(MSG_DIR));
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
    let (text, _) = build(&items, Path::new(MSG_DIR));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], base_line(1));
    assert!(lines[4].starts_with("fixup "), "{text}");
    assert_eq!(lines[5], exec_line(1));
    assert!(lines[6].starts_with("pick "));
}

#[test]
fn problems_with_the_list() {
    use RebaseAction::*;
    let list = |actions: &[RebaseAction]| actions.iter().enumerate().map(|(i, a)| item(i as u8, *a)).collect::<Vec<_>>();
    assert_eq!(problem(&list(&[Squash, Pick])).as_deref(), Some(FIRST_COMBINES));
    assert_eq!(problem(&list(&[Fixup])).as_deref(), Some(FIRST_COMBINES));
    assert_eq!(problem(&list(&[Drop, Drop, Squash])).as_deref(), Some(FIRST_COMBINES));
    assert_eq!(problem(&list(&[Drop, Pick, Squash, Fixup])), None);
    assert_eq!(problem(&[reword(1, "")]).as_deref(), Some(EMPTY_REWORD));
    assert_eq!(problem(&[reword(1, " \n\t ")]).as_deref(), Some(EMPTY_REWORD));
    assert_eq!(problem(&[item(1, Reword)]), None, "a reword without a typed message keeps the original");
}

#[test]
fn unchanged_lists() {
    let original = vec![item(1, RebaseAction::Pick), item(2, RebaseAction::Pick)];
    assert!(is_unchanged(&original, &original));
    assert!(!is_unchanged(&[original[1].clone(), original[0].clone()], &original));
    let mut edited = original.clone();
    edited[1].action = RebaseAction::Reword;
    assert!(is_unchanged(&edited, &original), "a reword keeping its message changes nothing");
    edited[1].new_message = Some("Other".into());
    assert!(!is_unchanged(&edited, &original));
    edited[1] = item(2, RebaseAction::Drop);
    assert!(!is_unchanged(&edited, &original));
}

#[test]
fn quoting_and_editor_commands() {
    assert_eq!(shell_quote_for("C:\\Program Files\\Trellis\\trellis.exe", true), "'C:/Program Files/Trellis/trellis.exe'");
    // On Unix a backslash is part of a name: `/tmp/a\b` is not `/tmp/a/b`.
    assert_eq!(shell_quote_for("/tmp/a\\b/todo", false), "'/tmp/a\\b/todo'");
    assert_eq!(shell_quote("it's"), "'it'\\''s'");
    assert_eq!(shell_quote("/repo/x"), "'/repo/x'");
    if cfg!(windows) {
        let prepared = Path::new("C:\\repo\\.git\\trellis-rebase\\todo");
        let exe = SequenceEditor::Helper("C:\\Apps\\trellis.exe".into());
        assert_eq!(editor_command(&exe, prepared), "'C:/Apps/trellis.exe' --write-todo 'C:/repo/.git/trellis-rebase/todo'");
        assert_eq!(editor_command(&SequenceEditor::Copy, prepared), "cp 'C:/repo/.git/trellis-rebase/todo'");
    }
    let prepared = Path::new("/repo/.git/trellis-rebase/todo");
    let exe = SequenceEditor::Helper("/opt/trellis/trellis".into());
    assert_eq!(editor_command(&exe, prepared), "'/opt/trellis/trellis' --write-todo '/repo/.git/trellis-rebase/todo'");
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
    let dir = crate::paths::temp_dir().join(format!("trellis-test-install-todo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (prepared, git_todo) = (dir.join("todo"), dir.join("git-rebase-todo"));
    let short = |n: u8| oid(n).to_string()[..7].to_owned();
    let git_text = format!("pick {} commit 1\npick {} commit 2\n\n# Rebase onto ...\n", short(1), short(2));
    let (ours, _) = build(&[item(2, RebaseAction::Pick), item(1, RebaseAction::Drop)], Path::new(MSG_DIR));
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
