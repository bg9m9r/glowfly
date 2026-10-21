//! The listing with ~/.ssh/config: keys the config names, where they go in the list, and what
//! the entries that aren't pairs point at.

use super::tests::temp;
use super::*;
use std::path::{Path, PathBuf};

fn no_env(_: &str) -> Option<String> {
    None
}

fn listed(home: &Path, extra: &[PathBuf]) -> SshListing {
    let cx = ExpandCx { home, user: "ada", uid: None, env: &no_env, windows: cfg!(windows) };
    list_with(home, None, extra, &cx).unwrap()
}

fn pair(dir: &Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), "private").unwrap();
    std::fs::write(dir.join(format!("{name}.pub")), format!("ssh-ed25519 AAAA{name} {name}\n")).unwrap();
}

/// `~/<rest>` as the scan spells it.
fn under(home: &Path, rest: &str) -> PathBuf {
    PathBuf::from(format!("{}/{rest}", home.to_string_lossy()))
}

#[test]
fn config_keys_are_listed_between_folder_keys_and_extras() {
    let home = temp("list-config");
    let ssh = home.join(".ssh");
    pair(&ssh, "id_ed25519");
    pair(&ssh, "a_key");
    pair(&home.join("work"), "id_work");
    pair(&home.join("deploy"), "deploy");
    std::fs::write(home.join("only.pub"), "ssh-ed25519 AAAAonly only\n").unwrap();
    std::fs::write(home.join("lone"), "private, no pub").unwrap();
    std::fs::create_dir_all(home.join("folder")).unwrap();
    let config = "IdentityFile ~/work/id_work\n\
                  Host github.com\n\
                  IdentityFile ~/.ssh/id_ed25519\n\
                  IdentityFile ~/work/id_work\n\
                  IdentityFile ~/gone\n\
                  IdentityFile ~/only\n\
                  IdentityFile ~/lone\n\
                  IdentityFile ~/folder\n\
                  IdentityFile ~/.ssh/%h\n";
    std::fs::write(ssh.join("config"), config).unwrap();
    let extra = [home.join("deploy").join("deploy"), home.join("work").join("id_work")];
    let got = listed(&home, &extra);
    let names: Vec<String> = got.keys.iter().map(SshKeyPair::file_name).collect();
    assert_eq!(names, ["a_key", "id_ed25519", "id_work", "deploy"], "folder, config, extras; each once");
    assert_eq!(got.config_file, ssh.join("config"));
    assert_eq!(got.config_error, None);
    let statuses: Vec<ConfigStatus> = got.config.iter().map(|c| c.status.clone()).collect();
    let work = under(&home, "work/id_work");
    assert_eq!(
        statuses,
        vec![
            ConfigStatus::Key(work.clone()),
            ConfigStatus::Key(ssh.join("id_ed25519")),
            ConfigStatus::Key(work),
            ConfigStatus::Missing(under(&home, "gone")),
            ConfigStatus::PublicOnly(under(&home, "only")),
            ConfigStatus::NotAPair(under(&home, "lone"), KeyFileProblem::NoPublicKey),
            ConfigStatus::NotAPair(under(&home, "folder"), KeyFileProblem::NotAFile),
            ConfigStatus::Unresolved(Unresolved::HostToken('h')),
        ]
    );
    let c = &got.config[1];
    assert_eq!((c.raw.as_str(), c.line, c.section.as_str()), ("~/.ssh/id_ed25519", 3, "Host github.com"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn an_unreadable_config_keeps_the_folder_keys() {
    let home = temp("list-config-error");
    let ssh = home.join(".ssh");
    pair(&ssh, "id_ed25519");
    std::fs::create_dir_all(ssh.join("config")).unwrap();
    let got = listed(&home, &[]);
    assert_eq!(got.keys.len(), 1);
    assert!(got.config_error.is_some());
    assert!(got.config.is_empty());
    let none = listed(&temp("list-config-none"), &[]);
    assert_eq!((none.config_error, none.config), (None, vec![]), "no config, no error");
    let _ = std::fs::remove_dir_all(&home);
}

/// Links `link` to the folder `target`: a symlink, or on Windows a junction when symlinks need
/// a privilege. False when neither can be made.
fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(target, link).is_ok()
        || std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(link).arg(target).output().is_ok_and(|o| o.status.success());
    made
}

#[test]
fn a_folder_key_the_config_spells_another_way_is_listed_once() {
    let home = temp("list-config-alias");
    let real = home.join("dotfiles").join("ssh");
    pair(&real, "id_ed25519");
    let ssh = home.join(".ssh");
    if !link_dir(&real, &ssh) {
        eprintln!("skipped: couldn't link a folder here");
        return;
    }
    let mut config = String::from("IdentityFile ~/dotfiles/ssh/id_ed25519\n");
    // A volume that ignores case finds the key by another case too.
    let upper = home.join("DOTFILES").join("SSH").join("id_ed25519");
    if upper.is_file() {
        config.push_str("IdentityFile ~/DOTFILES/SSH/id_ed25519\n");
    }
    std::fs::write(ssh.join("config"), &config).unwrap();
    let got = listed(&home, &[real.join("id_ed25519")]);
    let names: Vec<String> = got.keys.iter().map(SshKeyPair::file_name).collect();
    assert_eq!(names, ["id_ed25519"], "one card for one key file");
    assert_eq!(got.keys[0].private, ssh.join("id_ed25519"));
    assert_eq!(got.config.len(), config.lines().count());
    for c in &got.config {
        assert_eq!(c.status, ConfigStatus::Key(ssh.join("id_ed25519")), "{}", c.raw);
    }
    let _ = std::fs::remove_dir_all(&home);
}
