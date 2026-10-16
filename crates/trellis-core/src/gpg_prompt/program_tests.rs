use super::program::from_config;
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
