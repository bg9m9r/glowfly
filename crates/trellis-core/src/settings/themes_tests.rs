//! Theme files: parsing, aliases and loops, the starter file, and reading the folder.

use super::*;
use crate::settings::ThemeChoice;

fn p(text: &str) -> ThemeFile {
    parse("ocean", Path::new("themes/ocean.json"), text.as_bytes())
}

fn rgb(hex: u32) -> ColorValue {
    ColorValue::Rgb([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8])
}

#[test]
fn valid_file() {
    let f = p(r##"{ "name": "Ocean", "base": "light", "colors": { "accent": "#2fa4ff", "selected": "#123a5c" } }"##);
    assert_eq!((f.id.as_str(), f.name.as_str(), f.base), ("ocean", "Ocean", BuiltinTheme::Light));
    assert_eq!(f.colors.len(), 2);
    assert_eq!(f.colors["accent"], rgb(0x2fa4ff));
    assert_eq!(f.colors["selected"], rgb(0x123a5c));
    assert!(f.problems.is_empty(), "{:?}", f.problems);
    assert_eq!(f.file_name(), "ocean.json");
}

#[test]
fn name_and_base_default() {
    let f = p("{}");
    assert_eq!((f.name.as_str(), f.base), ("ocean", BuiltinTheme::Dark));
    assert!(f.problems.is_empty() && f.colors.is_empty());
    assert_eq!(p(r#"{"name": "  "}"#).name, "ocean", "a blank name is the stem");
}

#[test]
fn unknown_base_uses_dark() {
    let f = p(r##"{ "base": "Solar", "colors": { "accent": "#000000" } }"##);
    assert_eq!(f.base, BuiltinTheme::Dark);
    assert_eq!(f.problems, [ThemeProblem::UnknownBase("Solar".into())]);
    assert_eq!(f.colors["accent"], rgb(0), "colors still apply");
    assert_eq!(p(r#"{"base": "Dark"}"#).problems, [ThemeProblem::UnknownBase("Dark".into())], "ids are exact");
}

#[test]
fn unknown_color_is_ignored() {
    let f = p(r##"{ "colors": { "acent": "#2fa4ff", "accent": "#010203" } }"##);
    assert_eq!(f.problems, [ThemeProblem::UnknownColor("acent".into())]);
    assert!(!f.colors.contains_key("acent"));
    assert_eq!(f.colors["accent"], rgb(0x010203));
    assert_eq!(f.problems[0].to_string(), "\"acent\" is not a theme color; ignored");
}

#[test]
fn bad_values_name_their_key() {
    for (value, shown) in [
        (r##""#12345""##, r##""#12345""##),
        (r#""red-ish""#, r#""red-ish""#),
        (r##""#gggggg""##, r##""#gggggg""##),
        ("5", "5"),
        ("null", "null"),
    ] {
        let f = p(&format!(r#"{{ "colors": {{ "panel": {value} }} }}"#));
        assert_eq!(f.problems, [ThemeProblem::BadValue { key: "panel".into(), value: shown.into() }], "{value}");
        assert!(f.colors.is_empty());
    }
    let f = p(r##"{ "colors": { "panel": "#12345" } }"##);
    assert_eq!(f.problems[0].to_string(), r##""panel": "#12345" is not a #rrggbb color or a color name; using the base color"##);
}

#[test]
fn hex_ignores_case_and_whitespace() {
    let f = p(r##"{ "colors": { "bg": "  #AbCdEf ", "text": "#ABCDEF" } }"##);
    assert_eq!((f.colors["bg"], f.colors["text"]), (rgb(0xabcdef), rgb(0xabcdef)));
    assert_eq!(parse_hex("#+bcdef"), None);
    assert_eq!(parse_hex("abcdef"), None);
}

#[test]
fn aliases_resolve() {
    let f = p(r##"{ "colors": { "selected": "#123a5c", "hover": "selected", "border": " selected " } }"##);
    assert_eq!((f.colors["hover"], f.colors["border"]), (rgb(0x123a5c), rgb(0x123a5c)));
    let f = p(r#"{ "colors": { "hover": "selected" } }"#);
    assert_eq!(f.colors["hover"], ColorValue::Base("selected"), "to a key the file doesn't set");
    let f = p(r##"{ "colors": { "weak": "faint", "faint": "border", "border": "#0a0b0c" } }"##);
    assert_eq!(f.colors["weak"], rgb(0x0a0b0c), "chains are followed");
    assert!(f.problems.is_empty());
}

#[test]
fn border_keys_parse() {
    let f = p(r##"{ "colors": { "input_border": "#123456", "dialog_border": "border" } }"##);
    assert!(f.problems.is_empty(), "{:?}", f.problems);
    assert_eq!((f.colors["input_border"], f.colors["dialog_border"]), (rgb(0x123456), ColorValue::Base("border")));
    let f = p(r##"{ "colors": { "border": "#0a0b0c", "dialog_border": "border" } }"##);
    assert_eq!(f.colors["dialog_border"], rgb(0x0a0b0c));
}

#[test]
fn cycles_keep_the_base_value() {
    let f =
        p(r##"{ "colors": { "hover": "selected", "selected": "hover", "accent": "#010101", "removed": "removed", "weak": "hover" } }"##);
    let cycles: Vec<_> = f.problems.iter().filter(|p| matches!(p, ThemeProblem::Cycle { .. })).cloned().collect();
    let cycle = |k: &str| ThemeProblem::Cycle { key: k.into() };
    assert_eq!(cycles, [cycle("hover"), cycle("removed"), cycle("selected")]);
    for k in ["hover", "selected", "removed"] {
        assert!(!f.colors.contains_key(k), "{k}");
    }
    assert_eq!(f.colors["accent"], rgb(0x010101));
    assert_eq!(f.colors["weak"], ColorValue::Base("hover"), "an alias into a loop gets the base value");
    assert_eq!(f.problems.len(), 3);
    assert_eq!(cycles[0].key(), Some("hover"));
}

#[test]
fn legacy_color_names_still_load() {
    let f = p(r##"{ "colors": { "green": "#010203", "amber": "selected", "purple": "#040506", "hover": "blue" } }"##);
    assert!(f.problems.is_empty(), "{:?}", f.problems);
    assert_eq!(f.colors["added"], rgb(0x010203));
    assert_eq!(f.colors["modified"], ColorValue::Base("selected"));
    assert_eq!(f.colors["progress"], rgb(0x040506));
    assert_eq!(f.colors["hover"], ColorValue::Base("info"));
    assert!(!f.colors.contains_key("green"));
}

#[test]
fn new_color_name_wins_over_the_old_one() {
    let f = p(r##"{ "colors": { "added": "#010101", "green": "#020202" } }"##);
    assert!(f.problems.is_empty(), "{:?}", f.problems);
    assert_eq!(f.colors["added"], rgb(0x010101));
    assert_eq!(f.colors.len(), 1);
}

#[test]
fn file_level_problems() {
    let f = p("{ \"name\": \n");
    assert!(matches!(&f.problems[..], [ThemeProblem::Invalid(m)] if m.contains("line")), "{:?}", f.problems);
    assert_eq!((f.base, f.colors.len()), (BuiltinTheme::Dark, 0));
    assert_eq!(p("[1, 2]").problems, [ThemeProblem::NotAnObject]);
    assert_eq!(p(r#"{"colors": 5}"#).problems, [ThemeProblem::WrongType { field: "colors".into(), expected: "an object of colors" }]);
    assert_eq!(p(r#"{"base": true}"#).problems, [ThemeProblem::WrongType { field: "base".into(), expected: "a string" }]);
    assert_eq!(p(r#"{"author": "Ada"}"#).problems, [ThemeProblem::UnknownField("author".into())]);
}

#[test]
fn fatal_problems() {
    let fatal = [
        ThemeProblem::Unreadable("denied".into()),
        ThemeProblem::Invalid("x".into()),
        ThemeProblem::NotAnObject,
        ThemeProblem::TooLarge,
        ThemeProblem::UnknownBase("x".into()),
    ];
    assert!(fatal.iter().all(ThemeProblem::is_fatal));
    let ignored = [
        ThemeProblem::WrongType { field: "name".into(), expected: "a string" },
        ThemeProblem::UnknownField("x".into()),
        ThemeProblem::UnknownColor("x".into()),
        ThemeProblem::BadValue { key: "bg".into(), value: "5".into() },
        ThemeProblem::Cycle { key: "bg".into() },
    ];
    assert!(!ignored.iter().any(ThemeProblem::is_fatal));
}

#[test]
fn choices_and_builtins() {
    assert_eq!(COLOR_KEYS.len(), 46);
    for t in BuiltinTheme::ALL {
        assert_eq!(BuiltinTheme::from_id(t.id()), Some(t));
    }
    assert_eq!(ThemeChoice::System.label(), "Match system");
    assert_eq!(ThemeChoice::Sand.label(), "Trellis Sand");
    assert_eq!(ThemeChoice::Custom("ocean".into()).label(), "ocean");
    assert_eq!(ThemeChoice::HighContrastDark.palette_label(), "High contrast dark");
    let cat = ThemeCatalog { files: vec![p(r#"{"base": "light"}"#)], ..ThemeCatalog::default() };
    assert_eq!(cat.base_of(&ThemeChoice::Custom("ocean".into()), true), BuiltinTheme::Light);
    assert_eq!(cat.base_of(&ThemeChoice::Custom("gone".into()), false), BuiltinTheme::Dark);
    assert_eq!(cat.base_of(&ThemeChoice::System, false), BuiltinTheme::Light);
}

#[test]
fn starter_json_round_trips() {
    let colors: Vec<(&str, [u8; 3])> = COLOR_KEYS.iter().enumerate().map(|(i, k)| (*k, [i as u8, 255 - i as u8, (i * 3) as u8])).collect();
    let name = r#"My "best" \ theme"#;
    let text = starter_json(name, BuiltinTheme::Sand, &colors);
    assert!(text.ends_with("}\n"));
    let f = p(&text);
    assert!(f.problems.is_empty(), "{:?}", f.problems);
    assert_eq!((f.name.as_str(), f.base), (name, BuiltinTheme::Sand));
    for (k, c) in &colors {
        assert_eq!(f.colors[k], ColorValue::Rgb(*c), "{k}");
    }
    let at: Vec<usize> = COLOR_KEYS.iter().map(|k| text.find(&format!("\"{k}\":")).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "keys in palette order");
}

fn scratch(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-themes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn load_dir_reads_json_files() {
    let dir = scratch("load");
    let missing = load_dir(&dir);
    assert_eq!((missing.files.len(), missing.error.clone(), missing.dir.clone()), (0, None, Some(dir.clone())));
    assert!(!dir.exists(), "loading doesn't create the folder");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.json"), r#"{"name": "zebra"}"#).unwrap();
    std::fs::write(dir.join("B.JSON"), r#"{"name": "Apple"}"#).unwrap();
    std::fs::write(dir.join("c.json"), r#"{"name": "mango", "base": "sand"}"#).unwrap();
    std::fs::write(dir.join("notes.txt"), "{}").unwrap();
    std::fs::write(dir.join("sub").join("x.json"), "{}").unwrap();
    std::fs::write(dir.join("big.json"), vec![b' '; MAX_FILE_BYTES as usize + 1]).unwrap();
    let cat = load_dir(&dir);
    let ids: Vec<_> = cat.files.iter().map(|f| (f.id.as_str(), f.name.as_str())).collect();
    assert_eq!(ids, [("B", "Apple"), ("big", "big"), ("c", "mango"), ("a", "zebra")]);
    assert_eq!(cat.get("big").unwrap().problems, [ThemeProblem::TooLarge]);
    assert_eq!(cat.get("c").unwrap().base, BuiltinTheme::Sand);
    assert!(cat.get("A").is_none(), "ids are case-sensitive");
    assert!(cat.error.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_picks_a_free_name() {
    let dir = scratch("create");
    let colors = [("bg", [1, 2, 3]), ("text", [4, 5, 6])];
    let json = starter_json("Mine", BuiltinTheme::Light, &colors);
    let (id, path) = create(&dir, &json).unwrap();
    assert_eq!((id.as_str(), path.clone()), ("my-theme", dir.join("my-theme.json")));
    let (id2, _) = create(&dir, &json).unwrap();
    assert_eq!(id2, "my-theme-2");
    let cat = load_dir(&dir);
    let f = cat.get("my-theme").unwrap();
    assert_eq!((f.name.as_str(), f.base, f.colors["bg"], f.colors["text"]), ("Mine", BuiltinTheme::Light, rgb(0x010203), rgb(0x040506)));
    let _ = std::fs::remove_dir_all(&dir);
}
