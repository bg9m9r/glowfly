use super::*;
use std::path::PathBuf;

/// An absolute path (on every OS) to a program whose folder has a space.
fn exe() -> PathBuf {
    crate::paths::temp_dir().join("Program Files").join("Trellis").join("trellis.exe")
}

#[test]
fn the_windows_command_quotes_the_program_and_the_link() {
    let shown = crate::native_path::shown(&exe());
    assert_eq!(windows_command(&exe()), format!("\"{shown}\" \"%1\""));
    #[cfg(windows)]
    {
        let verbatim = PathBuf::from(format!(r"\\?\{shown}"));
        assert_eq!(windows_command(&verbatim), format!("\"{shown}\" \"%1\""), "no verbatim prefix");
    }
}

#[test]
fn the_desktop_entry_declares_the_scheme() {
    let text = desktop_entry(&exe()).unwrap();
    assert!(text.starts_with("[Desktop Entry]\n"), "{text}");
    assert!(text.contains("\nMimeType=x-scheme-handler/trellis;\n"), "{text}");
    assert!(text.contains("\nNoDisplay=true\n"), "{text}");
    let exec = text.lines().find_map(|l| l.strip_prefix("Exec=")).unwrap();
    assert!(exec.ends_with(" %u"), "{exec}");
    assert!(exec.starts_with('"'), "a path with a space is quoted: {exec}");
    assert!(desktop_entry(&crate::paths::temp_dir().join("a\nb")).is_err());
}

#[test]
fn desktop_exec_arguments_follow_the_quoting_rules() {
    assert_eq!(desktop_exec_arg("/usr/bin/trellis"), "/usr/bin/trellis");
    assert_eq!(desktop_exec_arg("/opt/my apps/trellis"), "\"/opt/my apps/trellis\"");
    // Inside quotes `$`, `` ` ``, `"` and `\` get a backslash, and then every backslash is
    // escaped again as a desktop-file string.
    assert_eq!(desktop_exec_arg("/opt/$x/t"), r#""/opt/\\$x/t""#);
    assert_eq!(desktop_exec_arg("/opt/a`b"), r#""/opt/a\\`b""#);
    assert_eq!(desktop_exec_arg("/opt/a\"b"), r#""/opt/a\\"b""#);
    assert_eq!(desktop_exec_arg(r"/opt/a\b"), r#""/opt/a\\\\b""#);
    // `%` starts a field code, so it is doubled, quoted or not.
    assert_eq!(desktop_exec_arg("/opt/100%/t"), "/opt/100%%/t");
    assert_eq!(desktop_exec_arg("/opt/100% sure/t"), "\"/opt/100%% sure/t\"");
}

#[test]
fn xdg_answers_are_read() {
    let ours = desktop_entry(&exe()).unwrap();
    assert_eq!(decide_xdg("", None, &exe()), SchemeStatus::NotRegistered);
    assert_eq!(decide_xdg("\n", Some(&ours), &exe()), SchemeStatus::NotRegistered);
    assert_eq!(decide_xdg("trellis-links.desktop\n", Some(&ours), &exe()), SchemeStatus::Registered);
    assert_eq!(decide_xdg("other-app.desktop", Some(&ours), &exe()), SchemeStatus::OtherProgram("other-app.desktop".into()));
    // Our file, but written for another copy of Trellis.
    let other = desktop_entry(&crate::paths::temp_dir().join("elsewhere").join("trellis")).unwrap();
    let exec = other.lines().find_map(|l| l.strip_prefix("Exec=")).unwrap().to_owned();
    assert_eq!(decide_xdg(DESKTOP_FILE, Some(&other), &exe()), SchemeStatus::OtherProgram(exec));
    // The default names our file, which is gone: desktops ignore it.
    assert_eq!(decide_xdg(DESKTOP_FILE, None, &exe()), SchemeStatus::NotRegistered);
}

#[test]
fn windows_answers_are_read() {
    let ours = windows_command(&exe());
    assert_eq!(decide_windows(None, &exe()), SchemeStatus::NotRegistered);
    assert_eq!(decide_windows(Some(""), &exe()), SchemeStatus::NotRegistered);
    assert_eq!(decide_windows(Some(&ours), &exe()), SchemeStatus::Registered);
    assert_eq!(decide_windows(Some(&ours.to_uppercase()), &exe()), SchemeStatus::Registered);
    let other = r#""D:\Apps\Other\other.exe" "%1""#;
    assert_eq!(decide_windows(Some(other), &exe()), SchemeStatus::OtherProgram(other.into()));
}
