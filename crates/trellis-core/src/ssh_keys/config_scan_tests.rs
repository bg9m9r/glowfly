//! Reading ~/.ssh/config and its Includes from fixture folders.

use super::tests::temp;
use super::*;
use std::path::{Path, PathBuf};

fn no_env(_: &str) -> Option<String> {
    None
}

/// Scans `home/.ssh/config` as the user ada.
fn scan(home: &Path) -> ConfigScan {
    let cx = ExpandCx { home, user: "ada", uid: None, env: &no_env, windows: cfg!(windows) };
    scan_config(&home.join(".ssh"), &cx)
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// (raw, file name, line, section) of each entry.
fn summary(s: &ConfigScan) -> Vec<(String, String, usize, String)> {
    s.entries
        .iter()
        .map(|e| (e.raw.clone(), e.file.file_name().unwrap().to_string_lossy().into_owned(), e.line, e.section.clone()))
        .collect()
}

fn entry(raw: &str, file: &str, line: usize, section: &str) -> (String, String, usize, String) {
    (raw.into(), file.into(), line, section.into())
}

#[test]
fn a_missing_config_is_empty_and_a_folder_is_an_error() {
    let home = temp("scan-missing");
    assert_eq!(scan(&home), ConfigScan::default());
    std::fs::create_dir_all(home.join(".ssh").join("config")).unwrap();
    let s = scan(&home);
    assert!(s.entries.is_empty());
    assert!(s.error.is_some(), "a folder named config can't be read");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn includes_are_read_in_place_and_in_lexical_order() {
    let home = temp("scan-include");
    let ssh = home.join(".ssh");
    let extra = home.join("extra.conf");
    let absolute = home.join("abs").join("abs.conf");
    let config = format!(
        "IdentityFile ~/.ssh/first\nInclude config.d/*.conf\nInclude ~/extra.conf \"{}\" missing.conf\nHost work\n  Include work.conf\n  IdentityFile ~/.ssh/last\n",
        absolute.to_string_lossy().replace('\\', "/")
    );
    write(&ssh.join("config"), &config);
    write(&ssh.join("config.d").join("b.conf"), "IdentityFile ~/.ssh/b\n");
    write(&ssh.join("config.d").join("a.conf"), "\nIdentityFile ~/.ssh/a\nHost a\nIdentityFile ~/.ssh/a2\n");
    write(&ssh.join("config.d").join("a.conf.bak"), "IdentityFile ~/.ssh/bak\n");
    write(&ssh.join("config.d").join(".hidden.conf"), "IdentityFile ~/.ssh/hidden\n");
    write(&extra, "IdentityFile ~/.ssh/extra\n");
    write(&absolute, "IdentityFile ~/.ssh/abs\n");
    write(&ssh.join("work.conf"), "IdentityFile ~/.ssh/work\nHost inner\nIdentityFile ~/.ssh/inner\n");
    let s = scan(&home);
    assert_eq!(s.error, None);
    assert_eq!(
        summary(&s),
        vec![
            entry("~/.ssh/first", "config", 1, ""),
            entry("~/.ssh/a", "a.conf", 2, ""),
            entry("~/.ssh/a2", "a.conf", 4, "Host a"),
            entry("~/.ssh/b", "b.conf", 1, ""),
            entry("~/.ssh/extra", "extra.conf", 1, ""),
            entry("~/.ssh/abs", "abs.conf", 1, ""),
            entry("~/.ssh/work", "work.conf", 1, "Host work"),
            entry("~/.ssh/inner", "work.conf", 3, "Host inner"),
            entry("~/.ssh/last", "config", 6, "Host work"),
        ]
    );
    assert_eq!(s.entries[1].file, ssh.join("config.d").join("a.conf"));
    assert_eq!(s.entries[0].path, Ok(PathBuf::from(format!("{}/.ssh/first", home.to_string_lossy()))));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn include_cycles_and_deep_nesting_stop() {
    let home = temp("scan-cycle");
    let ssh = home.join(".ssh");
    write(&ssh.join("config"), "Include a.conf\n");
    write(&ssh.join("a.conf"), "IdentityFile ~/.ssh/a\nInclude b.conf\n");
    write(&ssh.join("b.conf"), "IdentityFile ~/.ssh/b\nInclude a.conf config\n");
    let raws: Vec<String> = scan(&home).entries.into_iter().map(|e| e.raw).collect();
    assert_eq!(raws, ["~/.ssh/a", "~/.ssh/b"], "a file being read isn't read again");

    let deep = temp("scan-deep");
    let ssh = deep.join(".ssh");
    write(&ssh.join("config"), "IdentityFile ~/.ssh/k0\nInclude n1\n");
    for n in 1..=20 {
        write(&ssh.join(format!("n{n}")), &format!("IdentityFile ~/.ssh/k{n}\nInclude n{}\n", n + 1));
    }
    let raws: Vec<String> = scan(&deep).entries.into_iter().map(|e| e.raw).collect();
    assert_eq!(raws.len(), 17, "the config and 16 levels of Include");
    assert_eq!(raws.last().map(String::as_str), Some("~/.ssh/k16"));
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&deep);
}

#[test]
fn file_name_patterns() {
    assert!(name_matches("*.conf", "a.conf"));
    assert!(!name_matches("*.conf", "a.conf.bak"));
    assert!(name_matches("a?c", "abc"));
    assert!(!name_matches("a?c", "ac"));
    assert!(name_matches("*", "x"));
    assert!(!name_matches("*", ".hidden"), "a leading dot needs a dot");
    assert!(name_matches(".*", ".hidden"));
    assert!(name_matches("a*b*c", "aXbYbc"));
    assert!(!name_matches("A*", "a"), "case matters");
}
