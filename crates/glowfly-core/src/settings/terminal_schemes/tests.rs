//! Terminal schemes: importing Windows Terminal and iTerm2 files (fixtures in `fixtures/`),
//! the stored form, and reading and writing the schemes folder.

use super::io::stem_for;
use super::wt::{color, relaxed};
use super::*;

const HARBOUR_JSON: &[u8] = include_bytes!("fixtures/harbour.json");
const HARBOUR_ITERM: &[u8] = include_bytes!("fixtures/harbour.itermcolors");
const SETTINGS: &[u8] = include_bytes!("fixtures/settings.json");
const EIGHT_BIT: &[u8] = include_bytes!("fixtures/eightbit.itermcolors");
const BROKEN: &[u8] = include_bytes!("fixtures/broken.itermcolors");

fn hex(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

fn one(stem: &str, bytes: &[u8]) -> Scheme {
    let mut got = import(stem, bytes).unwrap();
    assert_eq!(got.schemes.len(), 1, "{got:?}");
    assert!(got.skipped.is_empty(), "{:?}", got.skipped);
    got.schemes.remove(0)
}

#[test]
fn keys() {
    assert_eq!(
        (ansi_key(0), ansi_key(1), ansi_key(8), ansi_key(15)),
        ("terminal_black", "terminal_red", "terminal_bright_black", "terminal_bright_white")
    );
    assert_eq!(TERMINAL_KEYS[..4], ["terminal_background", "terminal_foreground", "terminal_cursor", "terminal_selection"]);
}

#[test]
fn windows_terminal_scheme_sets_every_key() {
    let s = one("harbour", HARBOUR_JSON);
    assert_eq!(s.name, "Harbour Night");
    assert_eq!(s.colors.len(), 20, "every terminal key");
    assert_eq!(s.colors["terminal_background"], hex(0x101820));
    assert_eq!(s.colors["terminal_foreground"], hex(0xd6dae2));
    assert_eq!(s.colors["terminal_cursor"], hex(0xffcc66));
    assert_eq!(s.colors["terminal_selection"], hex(0x2a3a55));
    assert_eq!(s.colors["terminal_magenta"], hex(0xb47ce8), "purple is magenta");
    assert_eq!(s.colors["terminal_bright_magenta"], hex(0xd2a0ff));
    assert_eq!(s.colors["terminal_bright_white"], hex(0xf2f4f8));
}

#[test]
fn iterm_and_windows_terminal_agree() {
    let wt = one("harbour", HARBOUR_JSON);
    let it = one("Harbour Night", HARBOUR_ITERM);
    assert_eq!(it.name, "Harbour Night", "the file's name, as iTerm2 files have none");
    assert_eq!(it.colors, wt.colors, "float components round to the same bytes; Badge Color is ignored");
}

#[test]
fn settings_json_with_comments_imports_each_scheme() {
    let got = import("settings", SETTINGS).unwrap();
    let names: Vec<_> = got.schemes.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Paper", "settings 3"], "an unnamed scheme takes the file's name and its place");
    assert_eq!(got.skipped.len(), 1);
    assert!(got.skipped[0].contains("\"Broken\"") && got.skipped[0].contains("\"background\""), "{:?}", got.skipped);
    let paper = &got.schemes[0];
    assert_eq!(paper.colors.len(), 5, "missing keys stay unset, for the app theme to fill");
    assert_eq!(paper.colors["terminal_red"], hex(0xb3261e));
    assert_eq!(paper.colors["terminal_bright_black"], hex(0x808080), "alpha ignored");
    assert_eq!(paper.colors["terminal_white"], hex(0xcccccc), "#rgb");
    assert!(!paper.colors.contains_key("terminal_cursor"));
}

#[test]
fn iterm_eight_bit_components_and_partial_files() {
    let s = one("Paper Light", EIGHT_BIT);
    assert_eq!(s.colors.len(), 3);
    assert_eq!(s.colors["terminal_background"], hex(0xfdf6e3), "components above 1 are 0-255");
    assert_eq!(s.colors["terminal_foreground"], hex(0x3b4252), "an <integer> component");
    assert_eq!(s.colors["terminal_red"], hex(0xb02a2a), "alpha 0 is ignored");
}

#[test]
fn single_schemes_and_lists() {
    let s = one("x", br##"[{"name": "A", "red": "#ff0000"}]"##);
    assert_eq!((s.name.as_str(), s.colors["terminal_red"]), ("A", [255, 0, 0]));
    let s = one("x", "\u{feff}  {\"magenta\": \"#010203\", \"brightMagenta\": \"#040506\"}".as_bytes());
    assert_eq!((s.name.as_str(), s.colors["terminal_magenta"], s.colors["terminal_bright_magenta"]), ("x", hex(0x010203), hex(0x040506)));
}

#[test]
fn malformed_files_are_refused_with_a_reason() {
    let err = |b: &[u8]| import("x", b).unwrap_err();
    assert_eq!(err(b""), ImportError::Empty);
    assert_eq!(err(b"  \n"), ImportError::Empty);
    assert_eq!(err(b"bplist00\x01\x02"), ImportError::BinaryPlist);
    assert_eq!(err(b"hello"), ImportError::UnknownFormat);
    assert!(matches!(err(b"{\"name\": "), ImportError::Json(_)));
    assert_eq!(err(b"[]"), ImportError::NoScheme);
    assert_eq!(err(b"{\"schemes\": []}"), ImportError::NoScheme);
    assert!(matches!(err(b"{\"schemes\": 3}"), ImportError::Json(_)));
    assert_eq!(
        err(b"{\"name\": \"Nothing\", \"fontSize\": 12}").to_string(),
        "scheme \"Nothing\": it sets no terminal colors (background, foreground, black … brightWhite)"
    );
    let bad_hex = err(br##"{"background": "#12345g"}"##).to_string();
    assert_eq!(bad_hex, "\"background\" is \"#12345g\", not a #rrggbb color");
    assert!(matches!(err(br##"{"red": 7}"##), ImportError::Scheme(_)), "a number isn't a colour");
    assert!(matches!(err(BROKEN), ImportError::Plist(_)));
    assert!(matches!(err(b"<html><body></body></html>"), ImportError::Plist(_)));
    assert!(matches!(err(b"<plist><array></array></plist>"), ImportError::Plist(_)), "not a dict");
    assert!(matches!(err(b"<plist><dict></dict></plist>"), ImportError::Plist(_)), "no colours");
    let no_blue = b"<plist><dict><key>Ansi 1 Color</key><dict><key>Red Component</key><real>1</real><key>Green Component</key><real>0</real></dict></dict></plist>";
    assert!(err(no_blue).to_string().contains("no Blue Component"), "{}", err(no_blue));
    let nan = b"<plist><dict><key>Ansi 1 Color</key><dict><key>Red Component</key><real>x</real></dict></dict></plist>";
    assert!(matches!(err(nan), ImportError::Plist(_)));
    let deep = format!("<plist>{}{}</plist>", "<array>".repeat(100), "</array>".repeat(100));
    assert!(matches!(err(deep.as_bytes()), ImportError::Plist(_)), "deep nesting is refused, not followed");
}

#[test]
fn colours_and_relaxed_json() {
    assert_eq!(color("#AbCdEf"), Some(hex(0xabcdef)));
    assert_eq!(color(" #abc "), Some(hex(0xaabbcc)));
    assert_eq!(color("#11223344"), Some(hex(0x112233)));
    for bad in ["", "#", "abcdef", "#abcd", "#12345", "#ggg", "#+12345"] {
        assert_eq!(color(bad), None, "{bad}");
    }
    assert_eq!(relaxed("{\"a\": \"//x\", /* c */ \"b\": [1, 2,], // d\n}"), "{\"a\": \"//x\",   \"b\": [1, 2]}");
    assert_eq!(relaxed(r#"{"a": "q\"//,}"}"#), r#"{"a": "q\"//,}"}"#, "escapes in strings");
}

#[test]
fn stored_schemes_round_trip_and_report_problems() {
    let s = one("harbour", HARBOUR_JSON);
    let text = scheme_json(&s);
    let f = parse_stored("harbour-night", Path::new("harbour-night.json"), text.as_bytes());
    assert!(f.problems.is_empty(), "{:?}", f.problems);
    assert_eq!((f.name.as_str(), &f.colors), ("Harbour Night", &s.colors));
    let f = parse_stored(
        "x",
        Path::new("x.json"),
        br##"{"colors": {"terminal_red": "#ff0000", "red": "#00ff00", "terminal_blue": "blue"}, "base": "dark"}"##,
    );
    assert_eq!((f.name.as_str(), f.colors.len()), ("x", 1));
    assert_eq!(f.problems.len(), 3, "{:?}", f.problems);
    assert_eq!(parse_stored("x", Path::new("x.json"), b"[]").problems, [ThemeProblem::NotAnObject]);
    assert!(matches!(parse_stored("x", Path::new("x.json"), b"{").problems[..], [ThemeProblem::Invalid(_)]));
}

#[test]
fn file_stems() {
    assert_eq!(stem_for("One Half Dark"), "one-half-dark");
    assert_eq!(stem_for("  Ünïcode / Theme!! "), "n-code-theme");
    assert_eq!(stem_for("***"), "scheme");
    assert_eq!(stem_for("con"), "con-scheme", "a Windows device name");
    assert_eq!(stem_for(&"a".repeat(100)).len(), 60);
}

fn scratch(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-schemes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn import_path_saves_and_load_dir_lists() {
    let root = scratch("import");
    let dir = schemes_dir(&root);
    assert_eq!(load_dir(&dir), SchemeCatalog::default(), "a missing folder is empty");
    std::fs::create_dir_all(&root).unwrap();
    let src = root.join("settings.json");
    std::fs::write(&src, SETTINGS).unwrap();
    let report = import_path(&src, &dir).unwrap();
    assert_eq!(report.saved, [("paper".to_owned(), "Paper".to_owned()), ("settings-3".to_owned(), "settings 3".to_owned())]);
    assert_eq!(report.skipped.len(), 1);
    let again = import_path(&src, &dir).unwrap();
    assert_eq!(again.saved[0].0, "paper-2", "a taken name isn't overwritten");
    std::fs::write(dir.join("hand.JSON"), br##"{"name": "Hand", "colors": {"terminal_red": "#nope"}}"##).unwrap();
    std::fs::write(dir.join("notes.txt"), "{}").unwrap();
    let cat = load_dir(&dir);
    let ids: Vec<_> = cat.files.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, ["hand", "paper", "paper-2", "settings-3", "settings-3-2"]);
    assert_eq!(cat.get("paper").unwrap().colors["terminal_red"], hex(0xb3261e));
    assert_eq!(cat.get("hand").unwrap().problems.len(), 1);
    assert_eq!(crate::settings::themes::load_dir(&root).terminal, cat, "the themes folder reads its terminal folder");

    let big = root.join("big.json");
    std::fs::write(&big, vec![b' '; MAX_IMPORT_BYTES as usize + 1]).unwrap();
    assert_eq!(import_path(&big, &dir).unwrap_err(), ImportError::TooLarge);
    assert!(matches!(import_path(&root.join("gone.json"), &dir), Err(ImportError::Unreadable(_))));
    assert!(matches!(import_path(&root, &dir), Err(ImportError::Unreadable(_))), "a folder");
    let _ = std::fs::remove_dir_all(&root);
}
