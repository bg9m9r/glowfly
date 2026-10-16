//! Argument templates: splitting, filling in placeholders, and the per-tool placeholder lists.

use super::*;

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn os(v: &[&str]) -> Vec<OsString> {
    v.iter().map(OsString::from).collect()
}

/// `os`, with the paths in it spelled as `expand` fills them in on this system (`\` on Windows).
fn nat(v: &[&str]) -> Vec<OsString> {
    v.iter().map(|s| OsString::from(if cfg!(windows) { s.replace('/', "\\") } else { s.to_string() })).collect()
}

#[test]
fn expand_fills_in_this_systems_spelling() {
    let repo = Path::new("C:/x/y/");
    let file = repo.join("src/a.txt");
    let got = expand("{path} --cwd={repo}", &vars(&file, repo, 1)).unwrap();
    if cfg!(windows) {
        assert_eq!(got, os(&[r"C:\x\y\src\a.txt", r"--cwd=C:\x\y"]));
    } else {
        assert_eq!(got, os(&["C:/x/y/src/a.txt", "--cwd=C:/x/y"]));
    }
}

#[test]
fn split_separates_on_any_whitespace() {
    assert_eq!(split("a b").unwrap(), strs(&["a", "b"]));
    assert_eq!(split("  a\t\tb   c \n").unwrap(), strs(&["a", "b", "c"]));
    assert_eq!(split("").unwrap(), Vec::<String>::new());
    assert_eq!(split("   ").unwrap(), Vec::<String>::new());
}

#[test]
fn split_groups_quoted_text() {
    assert_eq!(split(r#"--goto "{path}:{line}""#).unwrap(), strs(&["--goto", "{path}:{line}"]));
    assert_eq!(split(r#"a"b c"d"#).unwrap(), strs(&["ab cd"]));
    assert_eq!(split(r#""""#).unwrap(), strs(&[""]));
    assert_eq!(split(r#"x "" y"#).unwrap(), strs(&["x", "", "y"]));
}

#[test]
fn split_keeps_backslashes() {
    assert_eq!(split(r"C:\dir\ D:\x").unwrap(), strs(&[r"C:\dir\", r"D:\x"]));
    assert_eq!(split(r#""C:\Program Files\" next"#).unwrap(), strs(&[r"C:\Program Files\", "next"]));
}

#[test]
fn split_refuses_an_unmatched_quote() {
    assert_eq!(split(r#"--goto "{path}"#), Err(TemplateError::UnmatchedQuote));
    assert_eq!(split(r#"""""#), Err(TemplateError::UnmatchedQuote));
    assert_eq!(TemplateError::UnmatchedQuote.to_string(), "unmatched quote");
}

fn vars<'a>(path: &'a Path, repo: &'a Path, line: u32) -> Vars<'a> {
    Vars { path, line, repo }
}

#[test]
fn expand_keeps_a_path_with_spaces_as_one_argument() {
    let path = Path::new("/home/me/my repo/a file.txt");
    let repo = Path::new("/home/me/my repo");
    assert_eq!(expand("{path}", &vars(path, repo, 1)).unwrap(), nat(&["/home/me/my repo/a file.txt"]));
    assert_eq!(expand("-n {repo}", &vars(path, repo, 1)).unwrap(), nat(&["-n", "/home/me/my repo"]));
}

#[test]
fn expand_fills_placeholders_inside_arguments() {
    let path = Path::new("/r/src/main.rs");
    let repo = Path::new("/r");
    let v = vars(path, repo, 42);
    assert_eq!(expand("--goto={path}:{line}", &v).unwrap(), nat(&["--goto=/r/src/main.rs:42"]));
    assert_eq!(expand(r#"--goto "{path}:{line}" --cwd={repo}"#, &v).unwrap(), nat(&["--goto", "/r/src/main.rs:42", "--cwd=/r"]));
    assert_eq!(expand("{line}{line}", &v).unwrap(), nat(&["4242"]));
}

#[test]
fn expand_is_single_pass() {
    // A repository folder literally named {line} is not expanded again.
    let repo = Path::new("/x/{line}");
    let path = Path::new("/x/{line}/{repo}.txt");
    assert_eq!(expand("{repo} {path}", &vars(path, repo, 7)).unwrap(), nat(&["/x/{line}", "/x/{line}/{repo}.txt"]));
}

#[test]
fn expand_keeps_unknown_names_and_lone_braces() {
    let p = Path::new("/r/a");
    let v = vars(p, Path::new("/r"), 1);
    assert_eq!(expand("{foo} { } {path {} }{line}", &v).unwrap(), nat(&["{foo}", "{", "}", "{path", "{}", "}1"]));
    assert_eq!(expand(r#"--x "{path}"#, &v), Err(TemplateError::UnmatchedQuote));
}

#[cfg(unix)]
#[test]
fn expand_keeps_non_utf8_paths() {
    use std::os::unix::ffi::OsStrExt;
    let raw = OsStr::from_bytes(b"/r/caf\xe9.txt");
    let args = expand("--file={path}", &vars(Path::new(raw), Path::new("/r"), 1)).unwrap();
    assert_eq!(args[0].as_bytes(), b"--file=/r/caf\xe9.txt");
}

#[test]
fn unknown_placeholders_lists_each_once() {
    let known = &EDITOR_PLACEHOLDERS;
    assert_eq!(unknown_placeholders("{path} {foo} {foo} {bar}", known), strs(&["{foo}", "{bar}"]));
    assert!(unknown_placeholders("{path}:{line} {repo}", known).is_empty());
    assert!(unknown_placeholders("{ } {} { path} {a-b}", known).is_empty(), "only {{identifier}} counts");
    assert_eq!(unknown_placeholders("--col={col}", known), strs(&["{col}"]));
}

#[test]
fn each_tool_has_its_own_placeholders() {
    assert_eq!(unknown_placeholders("{local} {remote} {path}:{line} {repo}", &DIFF_PLACEHOLDERS), strs(&["{line}"]));
    assert_eq!(unknown_placeholders("{base} {ours} {theirs} {output} {path}", &MERGE_PLACEHOLDERS), strs(&["{path}"]));
    assert_eq!(unknown_placeholders("{local}", &EDITOR_PLACEHOLDERS), strs(&["{local}"]));
}

#[test]
fn missing_placeholders_names_what_no_argument_uses() {
    let wanted = ["{ours}", "{theirs}", "{output}"];
    assert_eq!(missing_placeholders("{base} {ours} {theirs}", &wanted), strs(&["{output}"]));
    assert!(missing_placeholders("--out={output} {ours} {theirs}", &wanted).is_empty());
    assert_eq!(missing_placeholders("", &["{local}", "{remote}"]), strs(&["{local}", "{remote}"]));
    assert!(missing_placeholders(r#""{local}"#, &["{local}", "{remote}"]).is_empty(), "an unmatched quote is reported instead");
}

#[test]
fn expand_with_keeps_paths_with_spaces_as_one_argument() {
    let (old, new) = (Path::new("/tmp/t s/old/a b.txt"), Path::new("/r/my repo/a b.txt"));
    let got = expand_with("{local} --right={remote}", &[("{local}", Value::Path(old)), ("{remote}", Value::Path(new))]).unwrap();
    assert_eq!(got, nat(&["/tmp/t s/old/a b.txt", "--right=/r/my repo/a b.txt"]));
    // A value holding a placeholder's name is not expanded again.
    let tricky = Path::new("/x/{remote}.txt");
    let got = expand_with("{local} {remote}", &[("{local}", Value::Path(tricky)), ("{remote}", Value::Text("R".into()))]).unwrap();
    assert_eq!(got, nat(&["/x/{remote}.txt", "R"]));
}

#[test]
fn diff_and_merge_vars_fill_their_placeholders() {
    let (a, b, p, r) = (Path::new("/t/a"), Path::new("/t/b"), Path::new("/r/f.txt"), Path::new("/r"));
    let d = DiffVars { local: a, remote: b, path: p, repo: r };
    assert_eq!(
        d.expand("{local} {remote} -L {path} -C {repo} {line}").unwrap(),
        nat(&["/t/a", "/t/b", "-L", "/r/f.txt", "-C", "/r", "{line}"])
    );
    let m = MergeVars { base: Path::new("/t/base"), ours: a, theirs: b, output: p, repo: r };
    assert_eq!(m.expand(MERGE_DEFAULT).unwrap(), nat(&["/t/base", "/t/a", "/t/b", "/r/f.txt"]));
    assert_eq!(m.expand(r#"-o "{output}"#), Err(TemplateError::UnmatchedQuote));
}

const MERGE_DEFAULT: &str = "{base} {ours} {theirs} {output}";
