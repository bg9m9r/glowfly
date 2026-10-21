//! core.sshCommand: the value Glowfly writes, reading values back, and writing them with git.

use super::tests::temp;
use super::*;
use crate::paths::same_spelling;
use std::path::{Path, PathBuf};

#[test]
fn commands_quote_the_key() {
    let win = |p: &str| ssh_command_for_os(Path::new(p), true).unwrap();
    assert_eq!(win(r"C:\Users\ada\.ssh\id_ed25519"), "ssh -i 'C:/Users/ada/.ssh/id_ed25519' -o IdentitiesOnly=yes");
    assert_eq!(win("C:/Users/Ada O'Brien/.ssh/id"), r"ssh -i 'C:/Users/Ada O'\''Brien/.ssh/id' -o IdentitiesOnly=yes");
    // Elsewhere a backslash is part of a file name (on Windows it separates folders, so the
    // path itself is spelled differently there).
    #[cfg(unix)]
    {
        let unix = ssh_command_for_os(Path::new(r"/home/ada/odd\name"), false).unwrap();
        assert_eq!(unix, r"ssh -i '/home/ada/odd\name' -o IdentitiesOnly=yes", "a backslash stays");
    }
    #[cfg(windows)]
    assert_eq!(win(r"\\?\C:\Users\ada\.ssh\id"), "ssh -i 'C:/Users/ada/.ssh/id' -o IdentitiesOnly=yes");
}

#[cfg(unix)]
#[test]
fn a_path_that_isnt_utf8_has_no_command() {
    use std::os::unix::ffi::OsStrExt;
    let path = Path::new(std::ffi::OsStr::from_bytes(b"/home/ada/.ssh/\xff"));
    assert_eq!(ssh_command_for(path), None);
    let err = write_choice(Path::new("/"), false, Some(path), &[]).unwrap_err();
    assert_eq!(err, "This key's path can't be written into git config");
}

#[test]
fn reads_the_written_form_back() {
    for (path, windows) in [
        (r"C:\Users\ada\.ssh\id_ed25519", true),
        ("C:/Users/Ada O'Brien/.ssh/id", true),
        ("/home/ada/.ssh/id work", false),
        (r"/home/ada/odd\name", false),
    ] {
        let value = ssh_command_for_os(Path::new(path), windows).unwrap();
        let KeyChoice::Key(back) = parse_ssh_command(&value) else { panic!("{value}: not the Glowfly form") };
        let want = if windows { PathBuf::from(path.replace('\\', "/")) } else { PathBuf::from(path) };
        assert!(same_spelling(&back, &want), "{value}: {back:?}");
    }
}

#[test]
fn other_commands_are_flagged() {
    let other = |v: &str| match parse_ssh_command(v) {
        KeyChoice::Other { value, key } => {
            assert_eq!(value, v);
            key
        }
        c => panic!("{v}: {c:?}"),
    };
    assert_eq!(other("ssh -v"), None);
    assert_eq!(other("ssh -i ~/.ssh/k"), Some(PathBuf::from("~/.ssh/k")));
    assert_eq!(other("ssh -i ~/.ssh/k -o IdentitiesOnly=yes"), Some(PathBuf::from("~/.ssh/k")), "an unquoted ~ isn't Glowfly's");
    assert_eq!(other("ssh -i 'a' -o IdentitiesOnly=yes -v"), Some(PathBuf::from("a")));
    assert_eq!(other("ssh -i 'a"), None, "unbalanced quote");
    assert_eq!(other("ssh -i $HOME/.ssh/k -o IdentitiesOnly=yes"), None, "expanded by sh");
    assert_eq!(other("plink -batch"), None);
    assert_eq!(parse_ssh_command(""), KeyChoice::Unset);
    assert_eq!(parse_ssh_command("  \n"), KeyChoice::Unset);
}

#[test]
fn splits_like_sh() {
    let s = |t: &str| split_sh_words(t);
    let w = |v: &[&str]| Some(v.iter().map(|x| x.to_string()).collect::<Vec<_>>());
    assert_eq!(s("ssh  -i 'a b' -v"), w(&["ssh", "-i", "a b", "-v"]));
    assert_eq!(s(r#"ssh -i "a \"b\" \\ \$ \` \x""#), w(&["ssh", "-i", r#"a "b" \ $ ` \x"#]));
    assert_eq!(s(r"a\ b c\'d"), w(&["a b", "c'd"]));
    assert_eq!(s(r"'it'\''s'"), w(&["it's"]));
    assert_eq!(s("x=1#y"), w(&["x=1#y"]), "# inside a word is plain");
    assert_eq!(s(""), w(&[]));
    for bad in
        ["a 'b", "a \"b", r"a\", "a $b", "a `b`", "a; b", "a | b", "a && b", "a > b", "a < b", "(a)", "a*", "a?", "a[1]", "a #c", "\"$x\""]
    {
        assert_eq!(s(bad), None, "{bad}");
    }
}

/// A scratch repository and a global config file of its own, as `GIT_CONFIG_GLOBAL`.
fn fixture(name: &str) -> (PathBuf, PathBuf) {
    let dir = temp(name);
    let repo = dir.join("repo");
    git2::Repository::init(&repo).unwrap();
    (repo, dir.join("global.gitconfig"))
}

#[test]
fn writes_and_reads_each_level() {
    let (repo, global) = fixture("ssh-command");
    let global_s = global.to_string_lossy().into_owned();
    let env = [("GIT_CONFIG_GLOBAL", global_s.as_str()), ("GIT_CONFIG_NOSYSTEM", "1")];
    let key = repo.parent().unwrap().join(".ssh").join("id_ed25519");

    assert_eq!(read_choice(&repo, false, &env), Ok(KeyChoice::Unset), "exit code 1 is not an error");
    assert_eq!(read_choice(&repo, true, &env), Ok(KeyChoice::Unset));

    write_choice(&repo, false, Some(&key), &env).unwrap();
    let Ok(KeyChoice::Key(back)) = read_choice(&repo, false, &env) else { panic!("the local key") };
    assert!(same_spelling(&back, &key), "{back:?}");
    assert_eq!(read_choice(&repo, true, &env), Ok(KeyChoice::Unset), "the local write stays local");

    let other = repo.parent().unwrap().join(".ssh").join("id_work");
    write_choice(&repo, true, Some(&other), &env).unwrap();
    assert!(matches!(read_choice(&repo, true, &env), Ok(KeyChoice::Key(_))));
    assert!(std::fs::read_to_string(&global).unwrap().contains("id_work"), "written to the global file");
    let local = std::fs::read_to_string(repo.join(".git").join("config")).unwrap();
    assert!(!local.contains("id_work"), "{local}");

    write_choice(&repo, false, None, &env).unwrap();
    assert_eq!(read_choice(&repo, false, &env), Ok(KeyChoice::Unset));
    write_choice(&repo, false, None, &env).unwrap();
    write_choice(&repo, true, None, &env).unwrap();
    assert_eq!(read_choice(&repo, true, &env), Ok(KeyChoice::Unset));
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}
