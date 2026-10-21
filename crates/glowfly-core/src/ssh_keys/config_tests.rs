//! ssh config lines: splitting them as ssh does, the directives Glowfly reads, and expanding an
//! IdentityFile value by both Windows' and Unix rules (on every OS).

use super::*;
use std::path::{Path, PathBuf};

fn words(line: &str) -> Option<(String, Vec<String>)> {
    config_words(line)
}

fn kw(k: &str, args: &[&str]) -> Option<(String, Vec<String>)> {
    Some((k.to_owned(), args.iter().map(|a| a.to_string()).collect()))
}

#[test]
fn splits_lines_like_ssh() {
    assert_eq!(words("IdentityFile ~/.ssh/k"), kw("identityfile", &["~/.ssh/k"]));
    assert_eq!(words("  identityfile=~/.ssh/k"), kw("identityfile", &["~/.ssh/k"]));
    assert_eq!(words("IdentityFile = \"~/my keys/k\""), kw("identityfile", &["~/my keys/k"]));
    assert_eq!(words("IdentityFile '~/my keys/k'"), kw("identityfile", &["~/my keys/k"]));
    assert_eq!(words(r#"IdentityFile "a \"b\" c\\d""#), kw("identityfile", &[r#"a "b" c\d"#]));
    assert_eq!(words(r"IdentityFile a\ b"), kw("identityfile", &["a b"]));
    assert_eq!(words(r"IdentityFile C:\Users\ada\.ssh\k"), kw("identityfile", &[r"C:\Users\ada\.ssh\k"]), "other backslashes stay");
    assert_eq!(words("IdentityFile ~/.ssh/k # work"), kw("identityfile", &["~/.ssh/k"]), "a trailing comment");
    assert_eq!(words("IdentityFile a#b"), kw("identityfile", &["a#b"]), "# inside a word");
    assert_eq!(words("IDENTITYFILE ~/K"), kw("identityfile", &["~/K"]), "keywords ignore case, arguments don't");
    assert_eq!(words("Host a b\tc"), kw("host", &["a", "b", "c"]));
    for none in ["", "   ", "# a comment", "   # indented"] {
        assert_eq!(words(none), None, "{none:?}");
    }
    assert_eq!(words("IdentityFile \"~/unbalanced"), None, "ssh refuses the line");
    assert_eq!(words("IdentityFile 'x"), None);
}

#[test]
fn parses_the_directives_glowfly_reads() {
    let text = "IdentityFile ~/.ssh/top\n\
                # comment\n\
                Host github.com *.corp\n\
                \x20 IdentityFile ~/.ssh/gh extra\n\
                \x20 IdentityFile none\n\
                \x20 IDENTITYFILE NONE\n\
                \x20 User git\n\
                Match host x exec \"ssh-check\"\n\
                \x20 Include config.d/* ~/more.conf\n\
                \x20 IdentityFile \"\"\n";
    assert_eq!(
        parse_config(text),
        vec![
            Directive::Identity { line: 1, raw: "~/.ssh/top".into() },
            Directive::Section { line: 3, text: "Host github.com *.corp".into() },
            Directive::Identity { line: 4, raw: "~/.ssh/gh".into() },
            Directive::Section { line: 8, text: "Match host x exec ssh-check".into() },
            Directive::Include { line: 9, patterns: vec!["config.d/*".into(), "~/more.conf".into()] },
        ]
    );
}

fn no_env(_: &str) -> Option<String> {
    None
}

fn keys_env(name: &str) -> Option<String> {
    (name == "KEYS").then(|| "/srv/keys".to_owned())
}

/// Expands `raw` for the user ada, by Windows' rules (`windows`) or the Unix ones.
fn expand(raw: &str, windows: bool, uid: Option<u32>, env: &dyn Fn(&str) -> Option<String>) -> Result<PathBuf, Unresolved> {
    let home = if windows { Path::new("C:/Users/ada") } else { Path::new("/home/ada") };
    expand_identity(raw, &ExpandCx { home, user: "ada", uid, env, windows })
}

#[test]
fn expands_tilde_and_tokens() {
    for windows in [false, true] {
        let home = if windows { "C:/Users/ada" } else { "/home/ada" };
        let ok = |raw: &str| expand(raw, windows, Some(1000), &no_env).map(|p| p.to_string_lossy().into_owned());
        assert_eq!(ok("~"), Ok(home.to_owned()));
        assert_eq!(ok("~/x"), Ok(format!("{home}/x")));
        assert_eq!(ok("~ada/x"), Ok(format!("{home}/x")), "the current user");
        assert_eq!(ok("~bob/x"), Err(Unresolved::OtherUser("bob".into())));
        assert_eq!(ok("%d/k"), Ok(format!("{home}/k")));
        assert_eq!(ok("~/%u_key"), Ok(format!("{home}/ada_key")));
        assert_eq!(ok("~/100%%"), Ok(format!("{home}/100%")));
        assert_eq!(ok("~/k%i"), Ok(format!("{home}/k1000")));
        assert_eq!(expand("~/k%i", windows, None, &no_env), Err(Unresolved::Unsupported('i')));
        for t in ['h', 'r', 'p', 'n', 'C', 'k', 'j'] {
            assert_eq!(ok(&format!("~/.ssh/%{t}")), Err(Unresolved::HostToken(t)), "%{t}");
        }
        for t in ['L', 'l', 'z'] {
            assert_eq!(ok(&format!("~/.ssh/%{t}")), Err(Unresolved::Unsupported(t)), "%{t}");
        }
        assert_eq!(ok("~/.ssh/k%"), Err(Unresolved::Unsupported('%')));
        assert_eq!(ok("~/${KEYS"), Err(Unresolved::Unsupported('$')));
        assert_eq!(ok("${KEYS}/k"), Err(Unresolved::Variable("KEYS".into())));
        assert_eq!(ok("~/$HOME"), Ok(format!("{home}/$HOME")), "only ${{…}} is a variable");
        assert_eq!(ok("keys/k"), Err(Unresolved::Relative));
    }
    assert_eq!(expand("${KEYS}/k", false, None, &keys_env), Ok(PathBuf::from("/srv/keys/k")));
    assert_eq!(expand("${KEYS}/k", true, None, &keys_env), Err(Unresolved::NotWindowsPath));
}

#[test]
fn full_paths_by_each_os() {
    let win = |s: &str| absolute_for(s, true);
    assert_eq!(win("C:/k"), Ok("C:/k".into()));
    assert_eq!(win(r"C:\k"), Ok(r"C:\k".into()));
    assert_eq!(win("//srv/share/k"), Ok("//srv/share/k".into()));
    assert_eq!(win(r"\\srv\share\k"), Ok(r"\\srv\share\k".into()));
    assert_eq!(win("/c/Users/ada/k"), Ok("C:/Users/ada/k".into()), "Git for Windows' drive form");
    assert_eq!(win("/home/ada/k"), Err(Unresolved::NotWindowsPath));
    assert_eq!(win("keys/k"), Err(Unresolved::Relative));
    assert_eq!(win("C:k"), Err(Unresolved::Relative), "relative to the drive's folder");
    let unix = |s: &str| absolute_for(s, false);
    assert_eq!(unix("/home/ada/k"), Ok("/home/ada/k".into()));
    assert_eq!(unix("C:/k"), Err(Unresolved::Relative));
    assert_eq!(unix("keys/k"), Err(Unresolved::Relative));
    assert_eq!(expand("/c/Users/ada/k", true, None, &no_env), Ok(PathBuf::from("C:/Users/ada/k")));
}

#[test]
fn user_names_per_os() {
    let env = |n: &str| match n {
        "USERNAME" => Some("Ada".to_owned()),
        "LOGNAME" => Some("ada".to_owned()),
        _ => None,
    };
    assert_eq!(user_name_for(true, env), "Ada");
    assert_eq!(user_name_for(false, env), "ada", "LOGNAME when USER is unset");
    assert_eq!(user_name_for(false, |n| (n == "USER").then(|| "u".to_owned())), "u");
    assert_eq!(user_name_for(false, |_| None), "");
}
