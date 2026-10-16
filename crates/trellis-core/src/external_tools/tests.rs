//! Argument templates, finding programs and starting them.

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
    assert_eq!(unknown_placeholders("{path} {foo} {foo} {bar}"), strs(&["{foo}", "{bar}"]));
    assert!(unknown_placeholders("{path}:{line} {repo}").is_empty());
    assert!(unknown_placeholders("{ } {} { path} {a-b}").is_empty(), "only {{identifier}} counts");
    assert_eq!(unknown_placeholders("--col={col}"), strs(&["{col}"]));
}

#[test]
fn program_trims_and_unquotes() {
    assert_eq!(program("  code  "), Some(PathBuf::from("code")));
    assert_eq!(program(r#" "C:\Program Files\Ed\ed.exe" "#), Some(PathBuf::from(r"C:\Program Files\Ed\ed.exe")));
    assert_eq!(program(r#""ed""#), Some(PathBuf::from("ed")));
    assert_eq!(program(r#""""#), None);
    assert_eq!(program(""), None);
    assert_eq!(program(" \t "), None);
    assert_eq!(program(r#"" ""#), None);
}

/// Two PATH folders in a fresh temporary folder: the first empty, the second holding `files`.
fn path_fixture(name: &str, files: &[&str]) -> (PathBuf, OsString) {
    let root = crate::paths::temp_dir().join(format!("trellis-tools-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (a, b) = (root.join("one"), root.join("two"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    for f in files {
        std::fs::write(b.join(f), "").unwrap();
    }
    let joined = std::env::join_paths([a, b]).unwrap();
    (root, joined)
}

fn is_file(p: &Path) -> bool {
    p.is_file()
}

/// `is_file` with Windows' case-insensitive names, so the Windows lookup is tested the same way
/// on case-sensitive file systems (Linux), where `code` + `.CMD` must still find `code.cmd`.
fn is_file_any_case(p: &Path) -> bool {
    let (Some(dir), Some(name)) = (p.parent(), p.file_name()) else {
        return false;
    };
    let name = name.to_string_lossy().to_lowercase();
    std::fs::read_dir(dir)
        .is_ok_and(|entries| entries.flatten().any(|e| e.path().is_file() && e.file_name().to_string_lossy().to_lowercase() == name))
}

/// `found`'s text, lowercased: Windows file systems ignore case, so `code` + `.CMD` finds
/// `code.cmd` as `code.CMD`.
fn lower(found: Option<PathBuf>) -> Option<String> {
    found.map(|p| p.to_string_lossy().to_lowercase())
}

#[test]
fn resolve_finds_windows_wrappers_through_pathext() {
    let (root, path) = path_fixture("pathext", &["code.cmd", "tool.exe", "plain"]);
    let ext = OsStr::new(".COM;.EXE;.BAT;.CMD");
    let found = resolve(Path::new("code"), Some(&path), Some(ext), true, &is_file_any_case);
    assert_eq!(lower(found), lower(Some(root.join("two").join("code.cmd"))));
    // The default list when PATHEXT is unset.
    assert_eq!(
        lower(resolve(Path::new("tool"), Some(&path), None, true, &is_file_any_case)),
        lower(Some(root.join("two").join("tool.exe")))
    );
    // A name with an extension is not given another one.
    assert_eq!(resolve(Path::new("code.exe"), Some(&path), Some(ext), true, &is_file_any_case), None);
    assert_eq!(resolve(Path::new("tool.exe"), Some(&path), Some(ext), true, &is_file_any_case), Some(root.join("two").join("tool.exe")));
    // A bare name without an extension is only tried with PATHEXT's extensions on Windows.
    assert_eq!(resolve(Path::new("plain"), Some(&path), Some(ext), true, &is_file_any_case), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_on_unix_never_appends_extensions() {
    let (root, path) = path_fixture("unix", &["code.cmd", "ed"]);
    assert_eq!(resolve(Path::new("code"), Some(&path), Some(OsStr::new(".CMD")), false, &is_file), None);
    assert_eq!(resolve(Path::new("ed"), Some(&path), None, false, &is_file), Some(root.join("two").join("ed")));
    assert_eq!(resolve(Path::new("missing"), Some(&path), None, false, &is_file), None);
    assert_eq!(resolve(Path::new("ed"), None, None, false, &is_file), None, "no PATH, nothing found");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_takes_paths_only_if_they_exist() {
    let (root, path) = path_fixture("abs", &["ed"]);
    let full = root.join("two").join("ed");
    assert_eq!(resolve(&full, Some(&path), None, false, &is_file), Some(full.clone()));
    let gone = root.join("two").join("gone");
    assert_eq!(resolve(&gone, Some(&path), None, true, &is_file), None);
    // A relative path with a folder is not looked up on PATH.
    assert_eq!(resolve(Path::new("two/ed"), Some(&path), None, false, &is_file), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn command_line_opens_app_bundles_on_macos() {
    let args = os(&["/r/file.txt"]);
    let (p, a) = command_line(Path::new("/Applications/Ed.app"), &args, true);
    assert_eq!(p, PathBuf::from("open"));
    assert_eq!(a, os(&["-a", "/Applications/Ed.app", "/r/file.txt"]));
    assert_eq!(command_line(Path::new("/Applications/Ed.app"), &args, false), (PathBuf::from("/Applications/Ed.app"), args.clone()));
    assert_eq!(command_line(Path::new("/usr/bin/ed"), &args, true), (PathBuf::from("/usr/bin/ed"), args.clone()));
}

#[test]
fn a_terminal_keeps_its_console_handles() {
    // Nulled handles would leave a console shell reading NUL, so cmd would quit at once.
    let t = console_setup(true, false);
    assert!(!t.null_stdio);
    assert_eq!(t.creation_flags, CREATE_NEW_CONSOLE);
    // A batch-file terminal still gets a window of its own.
    assert_eq!(console_setup(true, true), t);
}

#[test]
fn an_editor_gets_no_input_output_or_wrapper_window() {
    assert_eq!(console_setup(false, false), ConsoleSetup { null_stdio: true, creation_flags: 0 });
    assert_eq!(console_setup(false, true), ConsoleSetup { null_stdio: true, creation_flags: CREATE_NO_WINDOW });
}

#[test]
fn spawn_reports_a_missing_program() {
    let l = Launch {
        program: PathBuf::from(format!("trellis-no-such-program-{}", std::process::id())),
        args: Vec::new(),
        cwd: crate::paths::temp_dir(),
        own_console: false,
    };
    let e = spawn(&l).unwrap_err();
    assert!(e.is_not_found(), "{e}");
    let l = Launch { program: crate::paths::temp_dir().join("trellis-no-such-dir").join("ed.exe"), ..l };
    assert!(spawn(&l).unwrap_err().is_not_found());
}

#[test]
fn spawn_starts_a_program_from_path() {
    let dir = crate::testutil::scratch_repo("tools-spawn");
    let l = Launch { program: PathBuf::from("git"), args: os(&["--version"]), cwd: dir.clone(), own_console: false };
    spawn(&l).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
