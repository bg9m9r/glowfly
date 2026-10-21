use super::program::{from_config, git_from, install_root, msys_candidates, pick, query};
use std::path::Path;

fn me() -> &'static Path {
    if cfg!(windows) { Path::new(r"C:\Apps\Glowfly\glowfly.exe") } else { Path::new("/opt/glowfly/bin/glowfly") }
}

fn me_text() -> String {
    me().display().to_string()
}

#[test]
fn the_users_program_is_the_last_that_isnt_glowfly() {
    let out = format!("gpg.program\nC:/GnuPG/bin/gpg.exe\0gpg.program\n{}\0", me_text());
    assert_eq!(from_config(&out, me()).as_deref(), Some("C:/GnuPG/bin/gpg.exe"));
    let out = format!("gpg.program\ngpg2\0gpg.openpgp.program\n/usr/local/bin/gpg\0gpg.program\n{}\0", me_text());
    assert_eq!(from_config(&out, me()).as_deref(), Some("/usr/local/bin/gpg"), "the later key wins");
}

#[test]
fn the_wrapper_asks_the_git_that_ran_it() {
    assert_eq!(git_from(None), "git");
    assert_eq!(git_from(Some("".into())), "git");
    assert_eq!(git_from(Some("/opt/git/bin/git".into())), "/opt/git/bin/git");
}

#[test]
fn nothing_usable_gives_none() {
    assert_eq!(from_config(&format!("gpg.program\n{}\0", me_text()), me()), None);
    assert_eq!(from_config("", me()), None);
    assert_eq!(from_config("gpg.program\ngpg2\0gpg.program\n\0", me()).as_deref(), Some("gpg2"), "an empty value is skipped");
    assert_eq!(from_config("gpg.program\0", me()), None, "a key without a value");
}

#[test]
#[cfg(windows)]
fn glowfly_is_recognised_in_another_spelling() {
    let out = "gpg.program\ngpg2\0gpg.program\nC:/Apps/Glowfly/glowfly.exe\0";
    assert_eq!(from_config(out, me()).as_deref(), Some("gpg2"));
}

/// git expands `~/` in `gpg.program` before running it, so the wrapper must run the expanded
/// path rather than look for a file literally named `~`.
#[test]
fn a_home_relative_program_is_expanded_like_git_does() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-gpg-program-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (home, repo) = (dir.join("home"), dir.join("repo"));
    std::fs::create_dir_all(&home).unwrap();
    git2::Repository::init(&repo).unwrap();
    let global = dir.join("global.gitconfig");
    std::fs::write(&global, "[gpg]\n\tprogram = ~/bin/my-gpg\n").unwrap();

    let got = query(me(), |cmd| {
        cmd.current_dir(&repo)
            .env("GIT_CONFIG_GLOBAL", &global)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &home)
            .env_remove("GIT_CONFIG_COUNT");
    });
    let want = home.join("bin").join("my-gpg");
    assert!(crate::paths::same_spelling(Path::new(&got), &want), "{got:?} is not {want:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn msys_paths_map_into_the_git_install() {
    let root = crate::paths::temp_dir().join("git-root");
    let usr_bin = vec![root.join("usr").join("bin").join("gpg"), root.join("bin").join("gpg")];
    assert_eq!(msys_candidates("/usr/bin/gpg", Some(&root)), usr_bin);
    assert_eq!(msys_candidates("/bin/gpg", Some(&root)), usr_bin, "/bin is /usr/bin");
    assert_eq!(msys_candidates("/mingw64/bin/gpg", Some(&root)), [root.join("mingw64").join("bin").join("gpg")]);
    assert_eq!(msys_candidates("/usr/local/bin/gpg", Some(&root)), [root.join("usr").join("local").join("bin").join("gpg")]);
    assert!(msys_candidates("/usr/bin/gpg", None).is_empty(), "no root");
    for value in ["gpg", "bin/gpg", "//server/share/gpg", "~/bin/gpg", ""] {
        assert!(msys_candidates(value, Some(&root)).is_empty(), "{value:?}");
    }
}

#[test]
#[cfg(windows)]
fn drive_forms_need_no_root() {
    use std::path::PathBuf;
    assert_eq!(msys_candidates("/c/Tools/gpg.exe", None), [PathBuf::from("C:/").join("Tools").join("gpg.exe")]);
    assert_eq!(msys_candidates("/cygdrive/d/x/gpg", None), [PathBuf::from("D:/").join("x").join("gpg")]);
    assert_eq!(msys_candidates("/C", None), [PathBuf::from("C:/")]);
    assert!(msys_candidates(r"C:\Tools\gpg.exe", None).is_empty(), "already a Windows path");
}

#[test]
fn install_root_from_exec_path() {
    let root = crate::paths::temp_dir().join("git-root");
    for exec in [
        root.join("mingw64").join("libexec").join("git-core"),
        root.join("clangarm64").join("libexec").join("git-core"),
        root.join("usr").join("lib").join("git-core"),
        root.join("MINGW64").join("libexec").join("git-core"),
    ] {
        assert_eq!(install_root(&exec), Some(root.clone()), "{exec:?}");
    }
    assert_eq!(install_root(&root.join("opt").join("git-core")), None);
    assert_eq!(install_root(Path::new("mingw64/libexec/git-core")), None, "relative");
}

#[test]
fn pick_prefers_an_existing_candidate() {
    let root = crate::paths::temp_dir().join("git-root");
    let candidates = vec![root.join("usr").join("bin").join("gpg"), root.join("bin").join("gpg")];
    let exe = root.join("bin").join("gpg.exe");
    assert_eq!(pick(&candidates, |p| p == exe), Some(&candidates[1]), "found with .exe");
    assert_eq!(pick(&candidates, |_| false), Some(&candidates[0]), "nothing exists: the first");
    assert_eq!(pick(&[], |_| true), None);
}

#[test]
#[cfg(windows)]
fn resolve_msys_finds_gpg_in_the_install() {
    use super::program::resolve_msys;
    let root = crate::paths::temp_dir().join(format!("glowfly-msys-root-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let bin = root.join("usr").join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("gpg.exe"), "").unwrap();
    let exec = Some(root.join("mingw64").join("libexec").join("git-core"));
    let want = crate::native_path::native(&bin.join("gpg")).display().to_string();
    assert_eq!(resolve_msys("/usr/bin/gpg".into(), exec.clone(), "git".as_ref()), want);
    assert_eq!(resolve_msys("/bin/gpg".into(), exec.clone(), "git".as_ref()), want);
    assert_eq!(resolve_msys("gpg2".into(), exec, "git".as_ref()), "gpg2");
    let _ = std::fs::remove_dir_all(&root);
}
