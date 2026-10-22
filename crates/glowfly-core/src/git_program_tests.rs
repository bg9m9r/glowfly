//! The chosen git program, checking a program before it is used, and finding installed gits.

use super::*;
use crate::external_tools::{find_on_path, locate_here};
use crate::paths::temp_dir;

/// A fresh folder for one test.
fn fixture(name: &str) -> PathBuf {
    let dir = temp_dir().join(format!("glowfly-git-program-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The name a `git` program file has here.
fn git_name() -> &'static str {
    if cfg!(windows) { "git.exe" } else { "git" }
}

/// Writes `text` to `p` and, on Unix, makes it runnable.
fn write_program(p: &Path, text: &str) {
    std::fs::write(p, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn known_locations_follow_each_os() {
    let (pf, la, home) = (temp_dir().join("pf"), temp_dir().join("la"), temp_dir().join("home"));
    let env = |k: &str| match k {
        "ProgramFiles" => Some(pf.clone().into_os_string()),
        "LOCALAPPDATA" => Some(la.clone().into_os_string()),
        "ProgramW6432" => Some(OsString::new()),
        _ => None,
    };
    let cmd_git = |base: PathBuf| base.join("cmd").join("git.exe");
    assert_eq!(
        known_locations(Os::Windows, &env, Some(&home)),
        [
            cmd_git(pf.join("Git")),
            cmd_git(la.join("Programs").join("Git")),
            cmd_git(home.join("scoop").join("apps").join("git").join("current"))
        ]
    );
    assert!(known_locations(Os::Windows, &|_| None, None).is_empty(), "nothing set, nothing guessed");

    let mac = known_locations(Os::MacOs, &|_| None, Some(&home));
    for p in ["/opt/homebrew/bin/git", "/Library/Developer/CommandLineTools/usr/bin/git", "/usr/bin/git"] {
        assert!(mac.contains(&PathBuf::from(p)), "{p}: {mac:?}");
    }
    let linux = known_locations(Os::Linux, &|_| None, Some(&home));
    assert!(linux.contains(&PathBuf::from("/usr/bin/git")), "{linux:?}");
    assert!(linux.contains(&home.join(".local").join("bin").join("git")), "{linux:?}");
}

#[test]
fn apple_shim_needs_developer_tools() {
    assert!(!apple_shim_usable(&|_| false));
    assert!(apple_shim_usable(&|p| p.starts_with("/Library/Developer")));
    assert!(apple_shim_usable(&|p| p.starts_with("/Applications/Xcode.app")));
}

#[test]
fn launchers_are_refused_by_name() {
    assert!(is_launcher(OsStr::new("git-bash.exe"), true));
    assert!(is_launcher(OsStr::new("Git-Bash.EXE"), true));
    assert!(is_launcher(OsStr::new("git-cmd.exe"), true));
    assert!(is_launcher(OsStr::new("gitk"), false));
    assert!(is_launcher(OsStr::new("git-gui"), false));
    assert!(!is_launcher(OsStr::new("git.exe"), true));
    assert!(!is_launcher(OsStr::new("git"), false));
    assert!(!is_launcher(OsStr::new("Git-Bash.exe"), false), "names are matched exactly outside Windows");
}

#[test]
fn every_git_on_path_once_in_order() {
    let dir = fixture("on-path");
    let (a, b, missing, c) = (dir.join("a"), dir.join("b"), dir.join("missing"), dir.join("c"));
    for d in [&a, &b] {
        std::fs::create_dir_all(d).unwrap();
        write_program(&d.join(git_name()), "");
    }
    let path_var = std::env::join_paths([&a, &missing, &b, &a]).unwrap();
    let pathext = OsString::from(".com;.exe;.bat;.cmd");
    let on_path = all_on_path(Some(&path_var), Some(&pathext), cfg!(windows), &locate_here);
    let (ga, gb) = (a.join(git_name()), b.join(git_name()));
    assert_eq!(on_path, [ga.clone(), gb.clone(), ga.clone()]);
    assert!(all_on_path(None, None, cfg!(windows), &locate_here).is_empty());

    let same = |x: &Path, y: &Path| crate::paths::resolve(x) == crate::paths::resolve(y);
    let known = vec![gb.clone(), c.join(git_name())];
    assert_eq!(gather(on_path, known, &locate_here, &same), [(ga, true), (gb.clone(), false)]);
    assert_eq!(gather(Vec::new(), vec![gb.clone()], &locate_here, &same), [(gb, false)], "only a PATH git is flagged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg(unix)]
fn a_link_and_its_target_are_one_candidate() {
    let dir = fixture("link");
    let (a, l) = (dir.join("a"), dir.join("l"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&l).unwrap();
    write_program(&a.join("git"), "");
    std::os::unix::fs::symlink(a.join("git"), l.join("git")).unwrap();
    let same = |x: &Path, y: &Path| crate::paths::resolve(x) == crate::paths::resolve(y);
    let got = gather(vec![l.join("git")], vec![a.join("git")], &locate_here, &same);
    assert_eq!(got, [(l.join("git"), true)], "the spelling found first is kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn probe_accepts_the_real_git() {
    let git = find_on_path(Path::new("git")).expect("git is on PATH");
    let v = probe(&git, &temp_dir()).expect("the PATH git works");
    assert_eq!(Some(v), gitcli::version(&temp_dir()));
}

#[test]
fn probe_refuses_with_a_reason() {
    let dir = fixture("refuse");
    let cwd = temp_dir();
    assert_eq!(probe(&dir.join("nothing").join(git_name()), &cwd), Err(Refusal::Missing));
    assert_eq!(probe(&dir, &cwd), Err(Refusal::Folder));

    // The test binary rejects `--version`: it runs, but isn't git.
    let me = std::env::current_exe().unwrap();
    let refusal = probe(&me, &cwd).expect_err("not git");
    assert!(matches!(refusal, Refusal::Failed { .. } | Refusal::NotGit(_)), "{refusal:?}");
    assert!(refusal.text().contains("isn't a working git"), "{}", refusal.text());

    // Refused by its name, so nothing runs (the file would fail to start if it did).
    let launcher = dir.join(if cfg!(windows) { "git-bash.exe" } else { "gitk" });
    write_program(&launcher, "not a program");
    assert_eq!(probe(&launcher, &cwd), Err(Refusal::Launcher));

    if cfg!(windows) {
        let text = dir.join("git.exe");
        std::fs::write(&text, "not a program").unwrap();
        let r = probe(&text, &cwd);
        assert!(matches!(r, Err(Refusal::CantRun(_))), "{r:?}");
    }
    #[cfg(unix)]
    {
        let plain = dir.join("git");
        std::fs::write(&plain, "#!/bin/sh\necho git version 2.40.1\n").unwrap();
        assert_eq!(probe(&plain, &cwd), Err(Refusal::CantRun("no permission to run it".into())));
    }
    for r in [Refusal::Missing, Refusal::Folder, Refusal::Launcher, Refusal::NotUnicode, Refusal::TimedOut] {
        assert!(!r.text().is_empty());
    }
    assert!(Refusal::Failed { code: Some(128), line: "fatal".into() }.text().contains("code 128 (fatal)"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `probe`, retried while Linux says the script just written is busy: another test thread may
/// have forked while the file was still open for writing, keeping a copy of the descriptor.
#[cfg(unix)]
fn probe_settled(p: &Path) -> Result<Version, Refusal> {
    for _ in 0..20 {
        match probe(p, &temp_dir()) {
            Err(Refusal::CantRun(why)) if why.contains("busy") => std::thread::sleep(Duration::from_millis(50)),
            other => return other,
        }
    }
    probe(p, &temp_dir())
}

#[test]
#[cfg(unix)]
fn probe_runs_a_fake_git() {
    let dir = fixture("fake");
    let git = dir.join("git");
    write_program(&git, "#!/bin/sh\necho 'git version 2.40.1'\n");
    assert_eq!(probe_settled(&git), Ok((2, 40, 1)));
    let other = dir.join("other").join("git");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    write_program(&other, "#!/bin/sh\necho hello\n");
    assert_eq!(probe_settled(&other), Err(Refusal::NotGit("hello".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn from_setting_keeps_only_absolute_paths() {
    assert_eq!(from_setting(None), None);
    assert_eq!(from_setting(Some("")), None);
    assert_eq!(from_setting(Some("   ")), None);
    assert_eq!(from_setting(Some("git")), None, "a bare name means git from PATH");
    let p = temp_dir().join("tools").join(git_name());
    assert_eq!(from_setting(Some(&format!("\"{}\"", p.display()))), Some(p));
}

#[test]
fn the_chosen_program_is_what_git_children_run() {
    // Only the real git is ever set, so tests running at the same time are unaffected.
    let real = find_on_path(Path::new("git")).expect("git is on PATH");
    assert!(real.is_absolute());
    let path_before = crate::env_path::path();
    set(Some(&real));
    assert_eq!(chosen().as_deref(), Some(real.as_path()));
    assert_eq!(program(), native(&real).into_os_string());
    assert!(gitcli::version(&temp_dir()).is_some());
    assert_eq!(crate::env_path::path(), path_before, "the PATH is left alone");
    set(None);
    assert_eq!(program(), "git");
}

#[test]
fn the_apple_shim_is_probed_only_with_developer_tools() {
    let shim = Path::new("/usr/bin/git");
    assert!(!safe_to_probe(shim, true, &|_| false), "it would open the install dialog");
    assert!(safe_to_probe(shim, true, &|_| true));
    assert!(safe_to_probe(shim, false, &|_| false), "elsewhere it is an ordinary git");
    assert!(safe_to_probe(Path::new("/opt/homebrew/bin/git"), true, &|_| false));
}

#[test]
fn the_path_git_is_found_and_checked() {
    let c = path_git(&temp_dir()).expect("git is on PATH");
    assert!(c.first_on_path);
    assert!(c.path.file_stem().is_some_and(|s| s.eq_ignore_ascii_case("git")), "{}", c.path.display());
    let v = c.check.expect("the PATH git works");
    assert_eq!(Some(v), gitcli::version(&temp_dir()));
}
