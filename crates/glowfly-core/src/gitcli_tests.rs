use super::*;

#[test]
fn caller_env_overrides_the_defaults() {
    let dir = crate::paths::temp_dir();
    let (ok, out) = run_with_input(&dir, &args(&["var", "GIT_EDITOR"]), "", &[("GIT_EDITOR", "foo")]);
    assert!(ok, "{out}");
    assert_eq!(out.trim(), "foo");
    let (ok, out) = run(&dir, &args(&["var", "GIT_EDITOR"]));
    assert!(ok, "{out}");
    assert_eq!(out.trim(), "true", "other calls keep the default");
}

#[test]
fn askpass_variables() {
    let a = AskpassEnv { exe: "C:/x/glowfly.exe".into(), addr: "127.0.0.1:1".into(), token: "ab".into() };
    let vars = askpass_vars(&a);
    let get = |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string_lossy().into_owned());
    assert_eq!(get("GIT_ASKPASS").as_deref(), Some("C:/x/glowfly.exe"));
    assert_eq!(get("SSH_ASKPASS").as_deref(), Some("C:/x/glowfly.exe"));
    assert_eq!(get("SSH_ASKPASS_REQUIRE").as_deref(), Some("force"));
    assert_eq!(get("GLOWFLY_ASKPASS").as_deref(), Some("127.0.0.1:1"));
    assert_eq!(get("GLOWFLY_ASKPASS_TOKEN").as_deref(), Some("ab"));
}

#[test]
fn no_prompts_fails_instead_of_asking() {
    let dir = crate::paths::temp_dir();
    let start = std::time::Instant::now();
    let a = args(&["-c", "credential.helper=", "credential", "fill"]);
    let (ok, out) = run_with_input(&dir, &a, "protocol=https\nhost=example.invalid\n\n", NO_PROMPTS);
    assert!(!ok, "{out}");
    assert!(start.elapsed() < std::time::Duration::from_secs(30));
}

#[test]
fn stdout_and_stderr_stay_apart() {
    let dir = crate::paths::temp_dir();
    let out = output(&dir, &args(&["version"]), "", &[], None);
    assert!(out.ok && out.code == Some(0) && out.stdout.starts_with("git version") && out.stderr.is_empty(), "{out:?}");
    let out = output(&dir, &args(&["config", "--no-such-option"]), "", &[], None);
    assert!(!out.ok && out.stdout.is_empty() && !out.stderr.is_empty(), "{out:?}");
    assert_eq!(out.combined(), out.stderr.trim_end());
}

#[test]
fn output_bytes_keeps_raw_stdout() {
    let dir = crate::paths::temp_dir();
    let a = args(&["-c", r"alias.b=!printf '\377\376'", "b"]);
    let (out, bytes) = output_bytes(&dir, &a, &[], None);
    assert!(out.ok, "{out:?}");
    assert_eq!(bytes, [0xFF, 0xFE]);
    assert!(out.stdout.is_empty(), "{out:?}");
}

#[test]
fn messages_are_untranslated() {
    let dir = crate::paths::temp_dir();
    let env = |k: &str| {
        let a = args(&["-c", &format!("alias.e=!echo ${k}"), "e"]);
        output(&dir, &a, "", &[("LANG", "de_DE.UTF-8")], None).stdout
    };
    assert_eq!(env("LANGUAGE").trim(), "C");
    assert_eq!(env("LC_MESSAGES").trim(), "C");
    assert_eq!(env("LANG").trim(), "de_DE.UTF-8", "the rest of the environment is kept");
    // The character set is the user's: hooks and gpg may print non-ASCII text.
    assert_eq!(env("LC_ALL").trim(), std::env::var("LC_ALL").unwrap_or_default());
}

#[test]
fn messages_stay_english_under_a_users_lc_all() {
    // Where a German locale and git's translations are installed, this would be German
    // without LANGUAGE=C; elsewhere it checks nothing breaks.
    let dir = crate::testutil::scratch_repo("gitcli-locale");
    for (k, v) in [("LC_ALL", "de_DE.UTF-8"), ("LANG", "de_DE.UTF-8")] {
        let out = output(&dir, &args(&["checkout", "no-such-branch-here"]), "", &[(k, v)], None);
        assert!(!out.ok);
        assert!(out.stderr.contains("did not match any"), "{k}: {}", out.stderr);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_openssh_gets_a_display_to_use_askpass() {
    use std::ffi::OsStr;
    assert_eq!(display_placeholder(None, true), Some(":0"));
    assert_eq!(display_placeholder(Some(OsStr::new("")), true), Some(":0"));
    assert_eq!(display_placeholder(Some(OsStr::new(":1")), true), None, "a real display is kept");
    assert_eq!(display_placeholder(None, false), None, "not with OpenSSH 8.4 or newer, or on Windows");
}

#[test]
fn only_old_openssh_gets_the_display_placeholder() {
    use std::ffi::OsStr;
    let a = AskpassEnv { exe: "glowfly".into(), addr: "127.0.0.1:1".into(), token: "ab".into() };
    let display = |legacy, d: Option<&str>| {
        let vars = askpass_vars_for(&a, legacy, d.map(OsStr::new));
        assert_eq!(vars.iter().filter(|(n, _)| *n == "SSH_ASKPASS_REQUIRE").count(), 1);
        vars.into_iter().find(|(n, _)| *n == "DISPLAY").map(|(_, v)| v.to_string_lossy().into_owned())
    };
    assert_eq!(display(true, None).as_deref(), Some(":0"));
    assert_eq!(display(true, Some("")).as_deref(), Some(":0"));
    assert_eq!(display(true, Some(":1")), None, "the real display is inherited");
    assert_eq!(display(false, None), None, "OpenSSH 8.4 or newer needs no display");
}

#[test]
fn background_jobs_never_prompt() {
    for pair in NO_PROMPTS {
        assert!(BACKGROUND.contains(pair), "{pair:?}");
    }
    assert!(BACKGROUND.contains(&("GCM_INTERACTIVE", "never")));
    assert!(!crate::gpg_prompt::env::allows_prompts(BACKGROUND));
}

#[test]
fn large_input_does_not_deadlock() {
    // Several MB in and out: writing stdin before draining stdout would block both sides.
    let dir = crate::paths::temp_dir();
    let input = ("x".repeat(64) + "\n").repeat(100_000);
    let a = args(&["-c", "alias.cat=!cat", "cat"]);
    let out = output(&dir, &a, &input, &[], Some(Duration::from_secs(60)));
    assert!(out.ok, "{}", out.stderr);
    assert_eq!(out.stdout.len(), input.len());
}

#[test]
fn reads_git_versions() {
    assert_eq!(parse_version("git version 2.54.0.windows.1\n"), Some((2, 54, 0)));
    assert_eq!(parse_version("git version 2.39.3 (Apple Git-146)"), Some((2, 39, 3)));
    assert_eq!(parse_version("git version 2.38.0.rc1"), Some((2, 38, 0)));
    assert_eq!(parse_version("git version 2.40"), Some((2, 40, 0)));
    for bad in ["", "hello", "git version", "git version x.y.z", "git version 2"] {
        assert_eq!(parse_version(bad), None, "{bad}");
    }
    let v = version(&crate::paths::temp_dir()).expect("git runs");
    assert!((v.0, v.1) >= crate::conflict_check::MIN_GIT, "{v:?}");
}

#[test]
fn timeout_kills_the_command() {
    let dir = crate::paths::temp_dir();
    let start = Instant::now();
    let a = args(&["-c", "alias.wait=!sleep 30", "wait"]);
    let out = output(&dir, &a, "", &[], Some(Duration::from_millis(300)));
    assert!(!out.ok && out.timed_out && out.code.is_none(), "{out:?}");
    assert!(out.stderr.contains("was stopped"), "{out:?}");
    assert!(start.elapsed() < Duration::from_secs(10), "{:?}", start.elapsed());
}

#[test]
fn command_carries_gitclis_environment() {
    let dir = crate::paths::temp_dir();
    let mut cmd = command(&dir, &args(&["--version"]), &[]);
    let env = |k: &str| cmd.get_envs().find(|(n, _)| *n == k).and_then(|(_, v)| v).map(|v| v.to_string_lossy().into_owned());
    assert_eq!(env("GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
    assert_eq!(env("LANGUAGE").as_deref(), Some("C"));
    let out = cmd.stdin(Stdio::null()).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("git version"), "{out:?}");
}

#[test]
fn a_stopped_command_ends_with_everything_it_started() {
    let dir = crate::testutil::scratch_repo("gitcli-stop");
    let marker = dir.join("late.txt");
    let shown = marker.display().to_string().replace('\\', "/");
    let alias = format!("alias.nap=!sleep 2 && echo late > '{shown}'");
    let a = args(&["-c", &alias, "nap"]);
    let stop = Stop::new();
    let asker = stop.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        asker.stop();
    });
    let started = std::time::Instant::now();
    let out = output_until(&dir, &a, "", NO_PROMPTS, &stop);
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
    assert!(out.stopped && !out.ok, "{out:?}");
    assert!(out.stderr.contains("was cancelled"), "{}", out.stderr);
    std::thread::sleep(Duration::from_secs(3));
    assert!(!marker.exists(), "the shell git started was stopped too");

    let early = Stop::new();
    early.stop();
    let out = output_until(&dir, &args(&["version"]), "", NO_PROMPTS, &early);
    assert!(out.stopped && !out.ok && out.stdout.is_empty(), "never started: {out:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unstopped_command_runs_as_usual() {
    let dir = crate::testutil::scratch_repo("gitcli-stop-none");
    let out = output_until(&dir, &args(&["rev-parse", "--is-inside-work-tree"]), "", NO_PROMPTS, &Stop::new());
    assert!(out.ok && !out.stopped && !out.forced, "{out:?}");
    assert_eq!(out.stdout.trim(), "true");
    let _ = std::fs::remove_dir_all(&dir);
}
