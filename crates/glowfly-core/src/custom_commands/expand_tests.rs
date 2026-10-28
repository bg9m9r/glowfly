use super::super::{Command, parse};
use super::*;
use std::path::PathBuf;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// The one command of kind `kind` with `template` (and the extra fields `extra`).
fn cmd(kind: &str, template: &str, extra: &str) -> Command {
    let text = format!("[[command]]\nname = \"C\"\nkind = \"{kind}\"\ntemplate = '{template}'\n{extra}\n");
    let (mut commands, problems) = parse(&text);
    assert!(problems.is_empty(), "{template}: {problems:?}");
    commands.remove(0)
}

/// An absolute folder on every OS.
fn repo() -> PathBuf {
    crate::paths::temp_dir().join("work").join("repo")
}

fn vars<'a>(repo: &'a std::path::Path, branch: Option<&'a str>, sha: Option<&'a str>, files: &'a [String]) -> Vars<'a> {
    Vars { repo, branch, sha, files }
}

fn git(template: &str, v: &Vars) -> Result<Vec<String>, Missing> {
    match cmd("git", template, "").expand(v, false)? {
        Expanded::Git(a) => Ok(a),
        other => panic!("{other:?}"),
    }
}

fn shell(template: &str, v: &Vars, windows: bool) -> String {
    match cmd("shell", template, "").expand(v, windows) {
        Ok(Expanded::Shell(s)) => s,
        other => panic!("{other:?}"),
    }
}

#[test]
fn git_templates_are_split_before_filling_in() {
    let r = repo();
    let files = ["a b.txt".to_owned(), "c.txt".to_owned()];
    let v = vars(&r, Some("main"), Some(SHA), &files);
    assert_eq!(git("log --oneline {sha}", &v).unwrap(), ["log", "--oneline", SHA]);
    assert_eq!(git("git log {branch}", &v).unwrap(), ["log", "main"], "a leading git word is dropped");
    assert_eq!(git("diff -- {files}", &v).unwrap(), ["--literal-pathspecs", "diff", "--", "a b.txt", "c.txt"]);
    assert_eq!(git("log \"--format=%h {branch}\"", &v).unwrap(), ["log", "--format=%h main"]);
    assert_eq!(git("-C {repo} status", &v).unwrap(), ["-C", crate::native_path::native(&r).to_str().unwrap(), "status"]);
    let odd = ["-rf".to_owned(), ":(glob)x".to_owned()];
    assert_eq!(git("add -- {files}", &vars(&r, None, None, &odd)).unwrap(), ["--literal-pathspecs", "add", "--", "./-rf", "./:(glob)x"]);
    let tricky = vars(&r, Some("{sha}"), Some(SHA), &files);
    assert_eq!(git("log {branch}", &tricky).unwrap(), ["log", "{sha}"], "values are not filled in again");
}

#[test]
fn git_templates_say_what_is_missing() {
    let r = repo();
    let none: [String; 0] = [];
    let v = vars(&r, None, None, &none);
    assert_eq!(git("diff {files}", &v), Err(Missing::Files));
    assert_eq!(git("log {branch}", &v), Err(Missing::Branch));
    assert_eq!(git("show {sha}", &v), Err(Missing::Sha));
    assert_eq!(cmd("git", "log {files} {sha} {branch}", "").missing(&v), Some(Missing::Branch), "branch first");
    assert_eq!(cmd("git", "log {files} {sha}", "").missing(&v), Some(Missing::Sha), "then sha");
    assert_eq!(cmd("git", "log", "").missing(&v), None);
}

#[test]
fn shell_values_are_single_quoted() {
    let r = repo();
    for windows in [false, true] {
        let files = ["it's $(rm -rf ~).txt".to_owned(), "-x".to_owned()];
        let v = vars(&r, Some("main"), Some(SHA), &files);
        assert_eq!(shell("echo {branch}", &v, windows), "echo 'main'");
        assert_eq!(shell("git show {sha}", &v, windows), format!("git show '{SHA}'"));
        assert_eq!(shell("ls {files}", &v, windows), "ls 'it'\\''s $(rm -rf ~).txt' './-x'");
        assert_eq!(shell("echo ${HOME} $HOME {branch}", &v, windows), "echo ${HOME} $HOME 'main'");
    }
    let none: [String; 0] = [];
    let v = vars(&r, None, None, &none);
    let native = crate::native_path::native(&r).to_string_lossy().into_owned();
    assert_eq!(shell("cd {repo}", &v, false), format!("cd {}", crate::rebase::shell_quote_for(&native, false)));
    let forward = shell("cd {repo}", &v, true);
    assert_eq!(forward, format!("cd '{}'", native.replace('\\', "/")));
    assert!(!forward.contains('\\'), "{forward}");
}

#[test]
fn previews_show_the_command() {
    let r = repo();
    let files = ["a b.txt".to_owned()];
    let v = vars(&r, Some("main"), Some(SHA), &files);
    let c = cmd("git", "log --format=%h -- {files}", "");
    assert_eq!(c.expand(&v, false).unwrap().preview(), "git --literal-pathspecs log --format=%h -- 'a b.txt'");
    assert_eq!(cmd("shell", "echo {branch}", "").expand(&v, false).unwrap().preview(), "echo 'main'");
}

#[test]
fn confirmations_fill_in_plainly() {
    let r = repo();
    let files = ["a.txt".to_owned(), "b c.txt".to_owned()];
    let v = vars(&r, Some("main"), Some(SHA), &files);
    let c = cmd("git", "reset --hard {sha}", "confirm = \"Reset {branch} to {sha} ({files})?\"");
    assert_eq!(c.confirm_text(&v), Some(ConfirmText::Text(format!("Reset main to {SHA} (a.txt, b c.txt)?"))));
    assert_eq!(cmd("git", "log", "confirm = true").confirm_text(&v), Some(ConfirmText::Default));
    assert_eq!(cmd("git", "log", "").confirm_text(&v), None);
}

#[test]
fn placeholders_skip_git_and_shell_braces() {
    let found: Vec<_> = placeholders("a {b} @{u} ${HOME} {x_1} {} {a-b} stash@{0}{sha}").map(|(_, p)| p).collect();
    assert_eq!(found, ["{b}", "{x_1}", "{sha}"]);
}

#[test]
fn git_files_are_literal_pathspecs() {
    let r = repo();
    let files = ["a[1].txt".to_owned(), "*.rs".to_owned()];
    let v = vars(&r, Some("main"), Some(SHA), &files);
    assert_eq!(git("add -- {files}", &v).unwrap(), ["--literal-pathspecs", "add", "--", "a[1].txt", "*.rs"]);
    assert_eq!(
        git("-C {repo} add {files}", &v).unwrap()[..3],
        ["--literal-pathspecs", "-C", crate::native_path::native(&r).to_str().unwrap()]
    );
    assert_eq!(git("log -- *.rs", &v).unwrap(), ["log", "--", "*.rs"], "without {{files}}, nothing changes");
    assert_eq!(git("log {sha}", &v).unwrap(), ["log", SHA]);
    for own in ["--glob-pathspecs", "--noglob-pathspecs", "--icase-pathspecs", "--literal-pathspecs"] {
        let t = format!("{own} add {{files}}");
        assert_eq!(git(&t, &v).unwrap(), [own, "add", "a[1].txt", "*.rs"], "the template's own pathspec option wins");
    }
    let s = shell("git add -- {files}", &v, false);
    assert_eq!(s, "git add -- 'a[1].txt' '*.rs'", "shell commands keep plain paths");
}

#[test]
fn a_git_command_touches_only_the_named_file() {
    let dir = crate::testutil::scratch_repo("custom-literal");
    for f in ["a[1].txt", "a1.txt", "b.txt"] {
        crate::testutil::write(&dir, f, "x\n");
    }
    let files = ["a[1].txt".to_owned()];
    let argv = git("add -- {files}", &vars(&dir, None, None, &files)).unwrap();
    let (ok, out) = crate::gitcli::run(&dir, &argv);
    assert!(ok, "{out}");
    let staged = crate::gitcli::read(&dir, &crate::gitcli::args(&["diff", "--cached", "--name-only", "-z"]));
    let names: Vec<&str> = staged.stdout.split('\0').filter(|n| !n.is_empty()).collect();
    assert_eq!(names, ["a[1].txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}
