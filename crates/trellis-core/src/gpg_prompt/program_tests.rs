use super::program::{from_config, git_from, query};
use std::path::Path;

fn me() -> &'static Path {
    if cfg!(windows) { Path::new(r"C:\Apps\Trellis\trellis.exe") } else { Path::new("/opt/trellis/bin/trellis") }
}

fn me_text() -> String {
    me().display().to_string()
}

#[test]
fn the_users_program_is_the_last_that_isnt_trellis() {
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
fn trellis_is_recognised_in_another_spelling() {
    let out = "gpg.program\ngpg2\0gpg.program\nC:/Apps/Trellis/trellis.exe\0";
    assert_eq!(from_config(out, me()).as_deref(), Some("gpg2"));
}

/// git expands `~/` in `gpg.program` before running it, so the wrapper must run the expanded
/// path rather than look for a file literally named `~`.
#[test]
fn a_home_relative_program_is_expanded_like_git_does() {
    let dir = crate::paths::temp_dir().join(format!("trellis-gpg-program-{}", std::process::id()));
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
