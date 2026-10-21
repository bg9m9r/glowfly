use super::*;

const FILE: FileFacts = FileFacts { is_dir: false, executable: false };
const EXEC: FileFacts = FileFacts { is_dir: false, executable: true };
const DIR: FileFacts = FileFacts { is_dir: true, executable: true };

/// (path, facts, system, PATHEXT, what it is when it runs)
type Case = (&'static str, Option<FileFacts>, Os, Option<&'static str>, Option<&'static str>);

#[test]
fn opens_as_program_table() {
    use Os::*;
    let table: &[Case] = &[
        // Windows: PATHEXT and the fixed list, any case, trailing dots and spaces ignored.
        ("run.bat", None, Windows, None, Some("a script")),
        ("tools/Setup.EXE", None, Windows, None, Some("a program")),
        ("run.bat.", None, Windows, None, Some("a script")),
        ("run.cmd .", None, Windows, None, Some("a script")),
        ("docs/link.lnk", None, Windows, None, Some("a shortcut")),
        ("site.url", None, Windows, None, Some("a shortcut")),
        ("app.hta", None, Windows, None, Some("a program")),
        ("saver.scr", None, Windows, None, Some("a program")),
        ("pkg.msi", None, Windows, None, Some("an installer")),
        ("build.ps1", None, Windows, None, Some("a script")),
        ("x.vbs", None, Windows, None, Some("a script")),
        ("x.js", None, Windows, None, Some("a script")),
        ("x.wsf", None, Windows, None, Some("a script")),
        ("x.jar", None, Windows, None, Some("a program")),
        ("x.reg", None, Windows, None, Some("a registry file")),
        // From PATHEXT only.
        ("x.pyw", None, Windows, Some(".COM;.EXE;.PYW"), Some("a program")),
        ("x.pyw", None, Windows, None, None),
        ("x.py", None, Windows, Some(".COM;.EXE;.PY"), Some("a program")),
        ("x.vbe", None, Windows, Some(""), Some("a script")),
        // Ordinary files, and names that only look like it.
        ("readme.md", None, Windows, None, None),
        ("notes.txt", Some(EXEC), Windows, None, None),
        ("bat", None, Windows, None, None),
        (".exe", None, Windows, None, None),
        ("file.exe.txt", None, Windows, None, None),
        // macOS: bundles, Terminal files, installers, and executable files without an extension.
        ("Tool.app", Some(DIR), MacOs, None, Some("a program")),
        ("Tool.APP", None, MacOs, None, Some("a program")),
        ("go.command", None, MacOs, None, Some("a script")),
        ("x.tool", None, MacOs, None, Some("a script")),
        ("x.terminal", None, MacOs, None, Some("a Terminal file")),
        ("x.pkg", None, MacOs, None, Some("an installer")),
        ("bin/run", Some(EXEC), MacOs, None, Some("an executable file")),
        ("bin/run", Some(FILE), MacOs, None, None),
        ("bin/run", None, MacOs, None, None),
        ("run.sh", Some(EXEC), MacOs, None, None),
        ("run.bat", None, MacOs, None, None),
        ("folder", Some(DIR), MacOs, None, None),
        // Linux: launchers and anything executable.
        ("app.desktop", None, Linux, None, Some("a launcher")),
        ("run.sh", Some(EXEC), Linux, None, Some("an executable file")),
        ("bin/run", Some(EXEC), Linux, None, Some("an executable file")),
        ("x.AppImage", Some(EXEC), Linux, None, Some("an executable file")),
        ("run.sh", Some(FILE), Linux, None, None),
        ("run.exe", Some(FILE), Linux, None, None),
        ("folder", Some(DIR), Linux, None, None),
        ("x.jar", None, Linux, None, Some("a program")),
    ];
    for &(path, facts, os, pathext, want) in table {
        assert_eq!(opens_as_program(Path::new(path), facts, os, pathext), want, "{path} {facts:?} on {os:?} with {pathext:?}");
    }
}

#[test]
fn the_current_system_is_one_of_the_three() {
    let os = Os::current();
    assert_eq!(os == Os::Windows, cfg!(windows));
    assert_eq!(os == Os::MacOs, cfg!(target_os = "macos"));
}

#[test]
fn facts_come_from_the_file_system() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-opens-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("run");
    std::fs::write(&file, "#!/bin/sh\n").unwrap();
    assert_eq!(FileFacts::of(&std::fs::metadata(&file).unwrap()), FILE);
    assert!(FileFacts::of(&std::fs::metadata(&dir).unwrap()).is_dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(FileFacts::of(&std::fs::metadata(&file).unwrap()), EXEC);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
