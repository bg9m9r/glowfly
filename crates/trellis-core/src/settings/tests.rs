//! Settings defaults, old files and migration.

use super::*;

#[test]
fn old_config_dir_moves_once() {
    let base = std::env::temp_dir().join(format!("trellis-migrate-{}", std::process::id()));
    let (old, new) = (base.join("Riffle"), base.join("Trellis"));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("recent.txt"), "C:/repo").unwrap();
    move_dir_if_new_missing(&old, &new).unwrap();
    assert!(!old.exists());
    assert_eq!(std::fs::read_to_string(new.join("recent.txt")).unwrap(), "C:/repo");

    // An existing new folder is never overwritten.
    std::fs::create_dir_all(&old).unwrap();
    move_dir_if_new_missing(&old, &new).unwrap();
    assert!(old.exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn settings_roundtrip_and_defaults() {
    // Missing fields fall back to defaults so old settings files keep working.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5, "date_format": "Iso"}"#).unwrap_or_default();
    assert_eq!(s.auto_fetch_minutes, 5);
    assert_eq!(s.default_branch, "main");
    assert_eq!(s.columns, GraphColumns::default());
    assert_eq!(Settings { initial_commits: 10, ..Settings::default() }.commit_limit(), MIN_INITIAL_COMMITS);
}

#[test]
fn diff_layout_defaults() {
    // Files written before the diff layout existed open in the inline layout.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.diff_layout, DiffLayout::Inline);
    let split = Settings { diff_layout: DiffLayout::Split, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&split).unwrap()).unwrap();
    assert_eq!(back.diff_layout, DiffLayout::Split);
}

#[test]
fn syntax_highlighting_defaults_on() {
    // Files written before the setting existed keep highlighting on.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.syntax_highlighting);
    let off = Settings { syntax_highlighting: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.syntax_highlighting);
}

#[test]
fn ignore_whitespace_defaults_off() {
    // Files written before the toggle existed show every whitespace change.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.diff_ignore_whitespace);
    let on = Settings { diff_ignore_whitespace: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.diff_ignore_whitespace);
}

#[test]
fn watch_files_defaults_on() {
    // Files written before the setting existed get live updates.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.watch_files);
    let off = Settings { watch_files: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.watch_files);
}

#[test]
fn diff_mode_defaults_to_hunks() {
    // Files written before the toggle existed show hunks only.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.diff_mode, DiffMode::Hunks);
    let whole = Settings { diff_mode: DiffMode::WholeFile, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&whole).unwrap()).unwrap();
    assert_eq!(back.diff_mode, DiffMode::WholeFile);
}

#[test]
fn editor_fields_default_for_old_files() {
    // Without a per-field serde default these fall back to `Settings::default()`, not 0/false.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.diff_font_size, 12.5);
    assert_eq!(s.tab_size, 4);
    assert!(s.diff_line_numbers);
    assert!(s.diff_word_highlights);
}

#[test]
fn editor_fields_roundtrip() {
    let s = Settings { diff_font_size: 16.5, tab_size: 8, diff_line_numbers: false, diff_word_highlights: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.diff_font_size, 16.5);
    assert_eq!(back.tab_size, 8);
    assert!(!back.diff_line_numbers);
    assert!(!back.diff_word_highlights);
}

#[test]
fn editor_values_are_clamped() {
    let font = |diff_font_size| Settings { diff_font_size, ..Settings::default() }.diff_font_px();
    assert_eq!(font(2.0), 8.0);
    assert_eq!(font(100.0), 24.0);
    assert_eq!(font(f32::NAN), DEFAULT_DIFF_FONT_SIZE);
    assert_eq!(font(f32::INFINITY), DEFAULT_DIFF_FONT_SIZE);
    assert_eq!(font(14.0), 14.0);
    let tab = |tab_size| Settings { tab_size, ..Settings::default() }.tab_columns();
    assert_eq!(tab(0), 1);
    assert_eq!(tab(200), 16);
    assert_eq!(tab(4), 4);
}

#[test]
fn push_after_commit_defaults_off() {
    // Files written before the checkbox existed don't push after committing.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.push_after_commit);
    let on = Settings { push_after_commit: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.push_after_commit);
}

#[test]
fn stage_renames_defaults_on() {
    // Files written before the checkbox existed stage renames of tracked files.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.stage_renames);
    let off = Settings { stage_renames: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.stage_renames);
}

#[test]
fn push_new_tags_defaults_off() {
    assert!(!Settings::default().push_new_tags);
    // Files written before the checkbox existed don't push new tags.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.push_new_tags);
    let on = Settings { push_new_tags: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.push_new_tags);
}

#[test]
fn delete_to_trash_defaults_on() {
    // Files written before the preference existed move deleted files to the trash.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.delete_to_trash);
    let off = Settings { delete_to_trash: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.delete_to_trash);
}

fn json(s: &Settings) -> serde_json::Value {
    serde_json::to_value(s).unwrap()
}

fn scratch_settings(name: &str, text: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("trellis-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.json");
    std::fs::write(&file, text).unwrap();
    (dir, file)
}

#[test]
fn torn_settings_file_is_backed_up_and_defaults_used() {
    let torn = br#"{"auto_fetch_minutes": 5, "connections": [{"acc"#;
    let (dir, file) = scratch_settings("torn", torn);
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(json(&s), json(&Settings::default()));
    let problem = problem.expect("a problem to report");
    assert_eq!(problem.reset, None);
    let backup = dir.join("settings.json.bak");
    assert_eq!(problem.backup.as_deref(), Some(backup.as_path()));
    assert!(problem.message().contains("settings.json.bak"));
    // The unreadable file is kept, and the next save can't overwrite it.
    assert_eq!(std::fs::read(&backup).unwrap(), torn);
    assert!(!file.exists());
    Settings::default().save_to(&file).unwrap();
    assert_eq!(std::fs::read(&backup).unwrap(), torn);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn garbage_bytes_are_backed_up() {
    let (dir, file) = scratch_settings("garbage", &[0xff, 0xfe, 0x00, 0x42]);
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(json(&s), json(&Settings::default()));
    assert!(problem.unwrap().backup.is_some());
    assert_eq!(std::fs::read(dir.join("settings.json.bak")).unwrap(), [0xff, 0xfe, 0x00, 0x42]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_value_resets_only_that_setting() {
    // A newer version's theme: the rest of the file still loads.
    let text = br#"{"auto_fetch_minutes": 7, "theme": "Sepia", "default_branch": "trunk"}"#;
    let (dir, file) = scratch_settings("unknown", text);
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(s.auto_fetch_minutes, 7);
    assert_eq!(s.default_branch, "trunk");
    assert_eq!(s.theme, Settings::default().theme);
    let problem = problem.unwrap();
    assert_eq!(problem.reset, Some(vec!["theme".to_owned()]));
    // The original is kept, and what could be read is written back.
    assert_eq!(std::fs::read(dir.join("settings.json.bak")).unwrap(), text);
    let (again, problem) = Settings::load_from(&file);
    assert_eq!((json(&again), problem), (json(&s), None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_is_not_a_problem() {
    let dir = std::env::temp_dir().join(format!("trellis-settings-missing-{}", std::process::id()));
    let (s, problem) = Settings::load_from(&dir.join("settings.json"));
    assert_eq!((json(&s), problem), (json(&Settings::default()), None));
}

#[test]
fn save_then_load_roundtrips() {
    let (dir, file) = scratch_settings("roundtrip", b"{}");
    let s = Settings { auto_fetch_minutes: 3, ..Settings::default() };
    s.save_to(&file).unwrap();
    let (back, problem) = Settings::load_from(&file);
    assert_eq!((json(&back), problem), (json(&s), None));
    let _ = std::fs::remove_dir_all(&dir);
}
