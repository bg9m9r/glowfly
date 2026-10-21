use super::*;

/// Whether sh is available here; says so when it isn't, so the caller skips.
fn have_sh() -> bool {
    match find_sh() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("skipping: {e}");
            false
        }
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis shell tool {name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn launch(script: &str, env: Vec<(&'static str, OsString)>, cwd: &Path) -> ShellLaunch {
    ShellLaunch { script: script.into(), env, cwd: cwd.to_owned() }
}

#[test]
fn commands_run_in_the_folder_with_their_variables() {
    if !have_sh() {
        return;
    }
    let dir = temp("run");
    std::fs::create_dir_all(dir.join("copies")).unwrap();
    let theirs = dir.join("copies").join("theirs.txt");
    std::fs::write(&theirs, "theirs\n").unwrap();
    std::fs::write(dir.join("a.txt"), "ours\n").unwrap();
    let env = merge_env(&dir.join("copies").join("base.txt"), &dir.join("copies").join("ours.txt"), &theirs, "a.txt");
    let r = run_shell_to_exit(&launch("cp \"$REMOTE\" \"$MERGED\"", env, &dir)).unwrap();
    assert!(r.success(), "{r:?}");
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "theirs\n");

    let r = run_shell_to_exit(&launch("echo oops >&2; exit 3", Vec::new(), &dir)).unwrap();
    assert_eq!(r, ExitReport { code: Some(3), said: "oops".into() });

    assert!(spawn_shell(&launch(":", Vec::new(), &dir)).is_ok());
    let e = spawn_shell(&launch("echo nope >&2; exit 4", Vec::new(), &dir)).unwrap_err().to_string();
    assert!(e.contains("code 4: nope"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn diff_and_merge_variables_follow_git() {
    let dir = crate::paths::temp_dir();
    let (local, remote) = (dir.join("old").join("a.txt"), dir.join("a.txt"));
    let env = diff_env(&local, &remote, "src/a.txt");
    let get = |env: &[(&'static str, OsString)], k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone());
    assert_eq!(get(&env, "LOCAL"), Some(native(&local).into_os_string()));
    assert_eq!(get(&env, "REMOTE"), Some(native(&remote).into_os_string()));
    assert_eq!(get(&env, "MERGED"), Some("src/a.txt".into()));
    assert_eq!(get(&env, "BASE"), Some("src/a.txt".into()), "git gives BASE = MERGED for a diff");
    let base = dir.join("base.txt");
    let env = merge_env(&base, &local, &remote, "src/a.txt");
    assert_eq!(get(&env, "BASE"), Some(native(&base).into_os_string()));
    assert_eq!(get(&env, "MERGED"), Some("src/a.txt".into()));
}
