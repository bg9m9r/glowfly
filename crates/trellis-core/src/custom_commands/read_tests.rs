//! Reading the repository's commands file from disk (the user's file lives in the real config
//! folder, so it isn't read here).

use super::*;

fn repo(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-commands-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(dir.join(".trellis")).unwrap();
    std::fs::write(repo_file(dir), bytes).unwrap();
}

fn kinds(r: &FileRead) -> Vec<ProblemKind> {
    r.problems.iter().map(|p| p.kind.clone()).collect()
}

#[test]
fn no_file_is_no_problem() {
    let dir = repo("none");
    let r = read_repo(&dir);
    assert_eq!(r, FileRead { path: Some(repo_file(&dir)), ..FileRead::default() });
    std::fs::create_dir_all(dir.join(".trellis")).unwrap();
    assert!(read_repo(&dir).problems.is_empty());
}

#[test]
fn a_file_is_read_and_parsed() {
    let dir = repo("read");
    write(&dir, b"\xef\xbb\xbf[[command]]\r\nname = \"Log\"\r\nkind = \"git\"\r\ntemplate = \"log\"\r\n");
    let r = read_repo(&dir);
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert_eq!(r.commands.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Log"]);
}

#[test]
fn large_and_non_utf8_files_are_refused() {
    let dir = repo("big");
    let mut big = b"# ".to_vec();
    big.resize(MAX_FILE_BYTES as usize + 1, b'x');
    write(&dir, &big);
    assert_eq!(kinds(&read_repo(&dir)), [ProblemKind::TooLarge]);
    write(&dir, b"name = \"\xff\"\n");
    assert_eq!(kinds(&read_repo(&dir)), [ProblemKind::NotUtf8]);
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(windows)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    if target.is_dir() { std::os::windows::fs::symlink_dir(target, at) } else { std::os::windows::fs::symlink_file(target, at) }
}

#[test]
fn links_are_refused() {
    let dir = repo("link");
    let outside = repo("link-outside");
    let elsewhere = outside.join("commands.toml");
    std::fs::write(&elsewhere, "[[command]]\nname = \"X\"\nkind = \"git\"\ntemplate = \"log\"\n").unwrap();
    std::fs::create_dir_all(dir.join(".trellis")).unwrap();
    if let Err(e) = link(&elsewhere, &repo_file(&dir)) {
        eprintln!("skipped: can't make a symlink here ({e})");
        return;
    }
    let r = read_repo(&dir);
    assert_eq!(kinds(&r), [ProblemKind::Link]);
    assert!(r.commands.is_empty(), "never followed");
    let dir = repo("link-folder");
    link(&outside, &dir.join(".trellis")).unwrap();
    assert_eq!(kinds(&read_repo(&dir)), [ProblemKind::Link]);
}
