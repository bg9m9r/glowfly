use super::super::{Expanded, Vars, parse};
use super::*;

/// A `find` that knows `known` (paths or bare names, each found as itself).
fn finder(known: Vec<PathBuf>) -> impl Fn(&Path) -> Option<PathBuf> {
    move |p: &Path| known.iter().any(|k| k == p).then(|| p.to_path_buf())
}

#[test]
fn sh_is_bin_sh_or_the_one_beside_git() {
    let root = crate::paths::temp_dir().join("trellis-sh-git");
    assert_eq!(sh_for(false, None, &finder(vec![])), Ok(PathBuf::from("/bin/sh")));
    let bin_sh = root.join("bin").join("sh.exe");
    for git in [root.join("cmd").join("git.exe"), root.join("bin").join("git.exe"), root.join("mingw64").join("bin").join("git.exe")] {
        assert_eq!(sh_for(true, Some(&git), &finder(vec![bin_sh.clone()])), Ok(bin_sh.clone()), "{git:?}");
    }
    let usr_sh = root.join("usr").join("bin").join("sh.exe");
    let git = root.join("cmd").join("git.exe");
    assert_eq!(sh_for(true, Some(&git), &finder(vec![usr_sh.clone()])), Ok(usr_sh));
    assert_eq!(sh_for(true, None, &finder(vec![PathBuf::from("sh.exe")])), Ok(PathBuf::from("sh.exe")), "PATH");
    assert_eq!(sh_for(true, Some(&git), &finder(vec![])), Err(ShellMissing));
}

/// A fresh folder for a run.
fn folder(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-sh-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Whether sh can run here; says so when it can't.
fn have_sh() -> bool {
    let found = find_sh().is_ok();
    if !found {
        eprintln!("skipped: no sh found");
    }
    found
}

#[test]
fn a_script_gets_its_values_literally_and_its_errors_in_order() {
    if !have_sh() {
        return;
    }
    let dir = folder("values");
    let text = "[[command]]\nname = \"C\"\nkind = \"shell\"\ntemplate = \"printf '%s|' {files}; echo err >&2; exit 3\"\n";
    let cmd = parse(text).0.remove(0);
    let files = ["a b".to_owned(), "$(x)".to_owned(), "it's".to_owned()];
    let v = Vars { repo: &dir, branch: None, sha: None, files: &files };
    let Ok(Expanded::Shell(script)) = cmd.expand(&v, cfg!(windows)) else { panic!("a shell command") };
    let (ok, out) = run_shell(&dir, &script);
    assert!(!ok, "exit 3 fails");
    assert!(out.starts_with(&format!("$ {script}\n")), "{out}");
    assert!(out.ends_with("a b|$(x)|it's|err"), "{out}");
}

#[test]
fn a_script_runs_in_its_folder_without_an_editor() {
    if !have_sh() {
        return;
    }
    let dir = folder("cwd");
    let (ok, out) = run_shell(&dir, "pwd");
    assert!(ok, "{out}");
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(out.trim_end().ends_with(&name), "{out}");
    // No -m: git would open an editor; GIT_EDITOR=true leaves the message empty instead.
    let script = "git init -q && git -c user.name=T -c user.email=t@example.com -c commit.gpgsign=false commit --allow-empty";
    let (ok, out) = run_shell(&dir, script);
    assert!(!ok, "an empty message aborts the commit: {out}");
}

#[test]
fn long_output_keeps_its_end() {
    let big = vec![b'x'; MAX_OUTPUT_BYTES * 3 + 10];
    let (kept, cut) = read_tail(&big[..]);
    assert!(cut);
    assert_eq!(kept.len(), MAX_OUTPUT_BYTES);
    let (kept, cut) = read_tail(&b"short"[..]);
    assert_eq!((kept.as_slice(), cut), (&b"short"[..], false));
}
