use super::*;
use std::path::PathBuf;

/// An absolute path (on every OS) to a program whose folder has a space.
fn exe() -> PathBuf {
    crate::paths::temp_dir().join("Program Files").join("Glowfly").join("glowfly.exe")
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
    assert!(text.contains("\nMimeType=x-scheme-handler/glowfly;x-scheme-handler/trellis;\n"), "{text}");
    assert!(text.contains("\nNoDisplay=true\n"), "{text}");
    let exec = text.lines().find_map(|l| l.strip_prefix("Exec=")).unwrap();
    assert!(exec.ends_with(" %u"), "{exec}");
    assert!(exec.starts_with('"'), "a path with a space is quoted: {exec}");
    assert!(desktop_entry(&crate::paths::temp_dir().join("a\nb")).is_err());
}

#[test]
fn desktop_exec_arguments_follow_the_quoting_rules() {
    assert_eq!(desktop_exec_arg("/usr/bin/glowfly"), "/usr/bin/glowfly");
    assert_eq!(desktop_exec_arg("/opt/my apps/glowfly"), "\"/opt/my apps/glowfly\"");
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
    assert_eq!(decide_xdg("glowfly-links.desktop\n", Some(&ours), &exe()), SchemeStatus::Registered);
    assert_eq!(decide_xdg("other-app.desktop", Some(&ours), &exe()), SchemeStatus::OtherProgram("other-app.desktop".into()));
    // Our file, but written for another copy of Glowfly.
    let other = desktop_entry(&crate::paths::temp_dir().join("elsewhere").join("glowfly")).unwrap();
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

/// A file of the repository's `packaging/` folder.
fn packaging(file: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_packaged_link_handler_is_glowfly_own_entry() {
    let packaged = packaging("linux/glowfly-links.desktop");
    assert_eq!(packaged, desktop_entry(std::path::Path::new(PACKAGED_EXE)).unwrap(), "regenerate it from url_scheme::desktop_entry");
    assert_eq!(decide_xdg(DESKTOP_FILE, Some(&packaged), std::path::Path::new(PACKAGED_EXE)), SchemeStatus::Registered);
    let app = packaging("linux/glowfly.desktop");
    for line in ["Exec=glowfly %f", "Icon=glowfly", "MimeType=inode/directory;", "StartupWMClass=glowfly"] {
        assert!(app.lines().any(|l| l == line), "{line} in glowfly.desktop");
    }
}

#[test]
fn the_installer_registers_what_glowfly_registers() {
    let wxs = packaging("windows/glowfly.wxs");
    // The installed program, as the registry sees it once the installer fills in the folder.
    let scheme = windows_command(std::path::Path::new("[INSTALLFOLDER]glowfly.exe"));
    assert_eq!(scheme, r#""[INSTALLFOLDER]glowfly.exe" "%1""#);
    assert_eq!(wxs.matches(&format!("Value='{scheme}'")).count(), 2, "the command of both schemes");
    let folder = scheme.replace("%1", "%V");
    assert_eq!(wxs.matches(&format!("Value='{folder}'")).count(), 2, "folders and folder backgrounds");
    for key in [
        r"Software\Classes\glowfly",
        r"Software\Classes\trellis",
        r"Software\Classes\Directory\shell\Glowfly",
        r"Software\Classes\Directory\Background\shell\Glowfly",
    ] {
        assert!(wxs.contains(&format!(r#"Root="HKCU" Key="{key}""#)), "{key} under HKCU");
    }
    assert_eq!(wxs.matches(r#"Value="URL:Glowfly link""#).count(), 2);
    assert!(wxs.contains(r#"Name="URL Protocol""#));
    assert!(wxs.contains(r#"Value='"[INSTALLFOLDER]glowfly.exe",0'"#), "the default icon");
    assert!(!wxs.contains("HKLM"), "nothing machine-wide");
}

#[test]
fn the_bundle_declares_the_scheme_and_folders() {
    let plist = packaging("macos/Info.plist");
    let lines: Vec<&str> = plist.lines().map(str::trim).collect();
    let after = |key: &str, n: usize| -> Vec<&str> {
        let at = lines.iter().position(|l| *l == format!("<key>{key}</key>")).unwrap_or_else(|| panic!("{key} in Info.plist"));
        lines.iter().skip(at + 1).take(n).copied().collect()
    };
    let schemes = [format!("<string>{SCHEME}</string>"), format!("<string>{OLD_SCHEME}</string>")];
    assert_eq!(after("CFBundleURLSchemes", 4), ["<array>", schemes[0].as_str(), schemes[1].as_str(), "</array>"]);
    let name = after("CFBundleURLName", 1);
    assert!(name[0].starts_with(&format!("<string>{}", crate::app_identity::APP_ID)), "{name:?}");
    assert_eq!(after("LSItemContentTypes", 3), ["<array>", "<string>public.folder</string>", "</array>"]);
    assert_eq!(after("CFBundleTypeRole", 1), ["<string>Viewer</string>"]);
    assert_eq!(after("LSHandlerRank", 1), ["<string>Alternate</string>"]);
    assert_eq!(plist.matches("<string>@VERSION@</string>").count(), 2, "both versions are filled in");
    let count = |tag: &str| lines.iter().filter(|l| **l == tag).count();
    assert_eq!(count("<dict>"), count("</dict>"), "dicts balance");
    assert_eq!(count("<array>"), count("</array>"), "arrays balance");
    let (open, close) = (plist.find("<!--").expect("a comment"), plist.find("-->").expect("its end"));
    assert!(!plist[open + 4..close].contains("--"), "an XML comment may not hold two dashes");
}
