use super::*;
use WinNameProblem::{Char, Device, Trailing};

#[test]
fn windows_name_problems() {
    let device = |d: &str| Some(Device(d.into()));
    let table: &[(&str, Option<WinNameProblem>)] = &[
        ("CON", device("CON")),
        ("con", device("con")),
        ("Prn.txt", device("Prn")),
        ("aux.c", device("aux")),
        ("nul.tar.gz", device("nul")),
        ("con .txt", device("con")),
        ("CONIN$", device("CONIN$")),
        ("conout$.log", device("conout$")),
        ("COM0", device("COM0")),
        ("com9.txt", device("com9")),
        ("LPT0", device("LPT0")),
        ("lpt5", device("lpt5")),
        ("COM¹", device("COM¹")),
        ("lpt².txt", device("lpt²")),
        ("COM³", device("COM³")),
        ("a<b", Some(Char('<'))),
        ("a>b", Some(Char('>'))),
        ("a:b", Some(Char(':'))),
        ("say\"hi\"", Some(Char('"'))),
        ("a|b", Some(Char('|'))),
        ("what?", Some(Char('?'))),
        ("*.txt", Some(Char('*'))),
        ("tab\there", Some(Char('\t'))),
        ("bell\u{7}", Some(Char('\u{7}'))),
        ("del\u{7f}", Some(Char('\u{7f}'))),
        ("notes.", Some(Trailing)),
        ("notes ", Some(Trailing)),
        ("notes...", Some(Trailing)),
        // Fine: not a device, or a device name inside a longer one.
        ("COM10", None),
        ("COM", None),
        ("LPT⁴", None),
        ("console.log", None),
        ("auxiliary", None),
        ("my-con", None),
        (".con", None),
        ("CONIN", None),
        (".gitignore", None),
        ("héllo wörld.txt", None),
        (" leading space", None),
        ("a.b.c", None),
    ];
    for (name, want) in table {
        assert_eq!(&win_name_problem(name), want, "{name:?}");
    }
}

#[test]
fn reasons_and_warnings() {
    assert_eq!(Device("aux".into()).reason(), "Windows reserves aux for a device");
    assert_eq!(Char('|').reason(), "Windows doesn't allow | in names");
    assert_eq!(Char('\n').reason(), "Windows doesn't allow control characters in names");
    assert_eq!(Trailing.reason(), "Windows drops a dot or space at the end of a name");
    assert_eq!(
        Trailing.warning("docs./a.md"),
        "Windows drops a dot or space at the end of a name, so docs./a.md won't check out on Windows."
    );
    assert_eq!(blocks_here(), cfg!(windows));
}

#[test]
fn portable_names_pass_the_windows_rules() {
    let table = [
        ("aux", "aux-wt"),
        ("CON", "CON-wt"),
        ("con.d", "con-wt.d"),
        ("con .d", "con-wt.d"),
        ("com¹", "com¹-wt"),
        ("a:b?c", "a-b-c"),
        ("name.", "name"),
        ("name. . ", "name"),
        ("fine", "fine"),
        ("console", "console"),
        ("...", ""),
    ];
    for (name, want) in table {
        let got = portable(name, "-wt");
        assert_eq!(got, want, "{name:?}");
        assert!(got.is_empty() || win_name_problem(&got).is_none(), "{got:?}");
    }
}

#[test]
fn path_problem_names_the_component() {
    assert_eq!(path_problem("src/aux/x.rs"), Some(("aux", Device("aux".into()))));
    assert_eq!(path_problem("a/b./c"), Some(("b.", Trailing)));
    assert_eq!(path_problem("src/lib.rs"), None);
}

#[test]
fn folding_matches_case_and_normalization() {
    assert_eq!(nfc("cafe\u{301}"), "caf\u{e9}");
    assert_eq!(fold("Ärger.TXT"), "ärger.txt");
    assert_eq!(fold("A\u{308}rger"), "ärger", "decomposed capital");
    assert!(same_folded("README.md", "readme.MD"));
    assert!(same_folded("Caf\u{e9}", "cafe\u{301}"));
    assert!(!same_folded("a.txt", "b.txt"));
}

#[test]
fn git_dir_look_alikes() {
    for name in [
        ".git",
        ".GIT",
        ".Git.",
        ".git ",
        ".git. .",
        "git~1",
        "GIT~1",
        ".git::$INDEX_ALLOCATION",
        ".g\u{200C}it",
        ".gi\u{200D}t",
        "\u{FEFF}.git",
        ".git\u{200E}",
        ".g\u{202A}it",
        ".g\u{206F}it",
        ".G\u{200F}IT",
    ] {
        assert!(is_git_dir_name(name), "{name:?}");
    }
    for name in [".gitignore", "git", ".github", "my.git", ".g\u{200B}it", "git~2"] {
        assert!(!is_git_dir_name(name), "{name:?}");
    }
    assert!(is_hfs_ignorable('\u{200C}') && is_hfs_ignorable('\u{FEFF}') && !is_hfs_ignorable('\u{200B}'));
}
