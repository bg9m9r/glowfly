//! The config stamp: what changes it, what must not, and what makes it never match.

use super::*;
use crate::testutil::{net_place, scratch_repo, set_config};
use std::path::Path;

fn fwd(p: &Path) -> String {
    p.display().to_string().replace(std::path::MAIN_SEPARATOR, "/")
}

fn append(file: &Path, text: &str) {
    use std::io::Write as _;
    std::fs::OpenOptions::new().create(true).append(true).open(file).unwrap().write_all(text.as_bytes()).unwrap();
}

/// A folder of its own, emptied, for the files a test includes.
fn folder(name: &str) -> PathBuf {
    let dir = paths::temp_dir().join(format!("glowfly-test-cfgstamp-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn stamp(git_dir: &Path) -> ConfigStamp {
    of_files(git_dir, &[])
}

#[test]
fn the_stamp_is_stable_until_the_repository_config_changes() {
    let dir = scratch_repo("cfgstamp-repo");
    let gd = dir.join(".git");
    let first = stamp(&gd);
    assert_eq!(first, stamp(&gd), "an untouched repository");
    set_config(&dir, &[("user.name", "X")]);
    let second = stamp(&gd);
    assert_ne!(first, second, "the repository's config changed");
    assert_eq!(second, stamp(&gd));
    std::fs::write(gd.join("config.worktree"), "[user]\n\tname = Y\n").unwrap();
    assert_ne!(second, stamp(&gd), "config.worktree appeared");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_included_file_and_its_creation_change_the_stamp() {
    let dir = scratch_repo("cfgstamp-include");
    let gd = dir.join(".git");
    let inc = folder("include").join("inc.cfg");
    append(&gd.join("config"), &format!("[include]\n\tpath = {}\n", fwd(&inc)));
    let missing = stamp(&gd);
    assert_eq!(missing, stamp(&gd));
    std::fs::write(&inc, "[user]\n\tname = A\n").unwrap();
    let created = stamp(&gd);
    assert_ne!(missing, created, "creating the included file");
    std::fs::write(&inc, "[user]\n\tname = B\n").unwrap();
    assert_ne!(created, stamp(&gd), "editing it");

    let branch = folder("branch").join("branch.cfg");
    append(&gd.join("config"), &format!("[includeIf \"onbranch:main\"]\n\tpath = {}\n", fwd(&branch)));
    let on_main = stamp(&gd);
    std::fs::write(gd.join("HEAD"), "ref: refs/heads/other\n").unwrap();
    assert_ne!(on_main, stamp(&gd), "HEAD is stamped under an onbranch include");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn outside_files_follow_git_s_environment() {
    let root = paths::temp_dir().join(format!("glowfly-test-cfgstamp-env-{}", std::process::id()));
    let (home, system, pd) = (root.join("home"), root.join("etc").join("gitconfig"), root.join("programdata"));
    let env_of =
        |vars: Vec<(&'static str, PathBuf)>| move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone().into_os_string());
    let with = |env: Vec<(&'static str, PathBuf)>, xdg: Option<PathBuf>| {
        outside_files_with(env_of(env), Some(&home), xdg, Some(system.clone()), Some(pd.clone()))
    };
    let (xdg_cfg, dot) = (home.join(".config").join("git").join("config"), home.join(".gitconfig"));
    assert_eq!(with(vec![], None), vec![pd.clone(), system.clone(), xdg_cfg.clone(), dot.clone()]);

    let g = root.join("global");
    assert_eq!(with(vec![("GIT_CONFIG_GLOBAL", g.clone())], None), vec![pd.clone(), system.clone(), g.clone()]);
    assert_eq!(with(vec![("GIT_CONFIG_NOSYSTEM", PathBuf::from("1"))], None), vec![xdg_cfg.clone(), dot.clone()]);
    assert_eq!(with(vec![("GIT_CONFIG_NOSYSTEM", PathBuf::from("0"))], None).len(), 4);
    let s = root.join("sys");
    assert_eq!(with(vec![("GIT_CONFIG_SYSTEM", s.clone())], None), vec![pd.clone(), s, xdg_cfg.clone(), dot.clone()]);

    let abs = root.join("xdg");
    let from_xdg = with(vec![("XDG_CONFIG_HOME", abs.clone())], None);
    assert_eq!(from_xdg[2], abs.join("git").join("config"));
    let relative = with(vec![("XDG_CONFIG_HOME", PathBuf::from("relative"))], None);
    assert_eq!(relative[2], xdg_cfg, "a relative XDG_CONFIG_HOME is ignored");
    let child = root.join("host-config");
    assert_eq!(with(vec![("XDG_CONFIG_HOME", abs)], Some(child.clone()))[2], child.join("git").join("config"));

    let extra = root.join("extra");
    assert_eq!(with(vec![("GIT_CONFIG", extra.clone())], None).last(), Some(&extra));
}

#[test]
fn a_global_file_edit_or_creation_changes_the_stamp() {
    let dir = scratch_repo("cfgstamp-global");
    let gd = dir.join(".git");
    let home = folder("global");
    let global = home.join(".gitconfig");
    let of = || of_files(&gd, std::slice::from_ref(&global));
    let missing = of();
    assert_eq!(missing, of());
    std::fs::write(&global, "[user]\n\tname = A\n").unwrap();
    let created = of();
    assert_ne!(missing, created);
    std::fs::write(&global, "[user]\n\tname = B\n").unwrap();
    let edited = of();
    assert_ne!(created, edited);

    let inc = home.join("more.cfg");
    std::fs::write(&global, format!("[include]\n\tpath = {}\n", fwd(&inc))).unwrap();
    let before = of();
    std::fs::write(&inc, "[user]\n\temail = a@b\n").unwrap();
    assert_ne!(before, of(), "a file the global one includes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_large_file_written_just_now_never_matches() {
    let dir = scratch_repo("cfgstamp-large");
    let gd = dir.join(".git");
    let inc = folder("large").join("big.cfg");
    std::fs::write(&inc, format!("[user]\n\tname = A\n# {}\n", "x".repeat(70 * 1024))).unwrap();
    append(&gd.join("config"), &format!("[include]\n\tpath = {}\n", fwd(&inc)));
    assert_ne!(stamp(&gd), stamp(&gd), "too recent to trust its time");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_include_on_another_network_place_never_matches() {
    let dir = scratch_repo("cfgstamp-net");
    let gd = dir.join(".git");
    assert_eq!(stamp(&gd), stamp(&gd));
    let far = net_place().join("x.cfg");
    append(&gd.join("config"), &format!("[include]\n\tpath = {}\n", fwd(&far)));
    assert_ne!(stamp(&gd), stamp(&gd));
    let _ = std::fs::remove_dir_all(&dir);
}
