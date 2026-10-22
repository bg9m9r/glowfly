//! Settings defaults, old files and migration.

use super::*;

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
fn load_more_on_scroll_defaults_on() {
    // Files written before the setting existed load more while scrolling.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.load_more_on_scroll);
    let off = Settings { load_more_on_scroll: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.load_more_on_scroll);
}

#[test]
fn index_history_defaults_on() {
    // Files written before the setting existed index large histories for search.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.index_history);
    let off = Settings { index_history: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.index_history);
}

#[test]
fn use_ssh_agent_defaults_off() {
    // Files written before the setting existed keep choosing keys through core.sshCommand.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.use_ssh_agent);
    assert!(!Settings::default().use_ssh_agent);
    let on = Settings { use_ssh_agent: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.use_ssh_agent);
}

#[test]
fn ssh_extra_keys_defaults_empty() {
    let s: Settings = serde_json::from_str(r#"{"use_ssh_agent": true}"#).unwrap();
    assert!(s.ssh_extra_keys.is_empty() && Settings::default().ssh_extra_keys.is_empty());
}

#[test]
fn git_program_defaults_to_path() {
    // Files written before the setting existed run git from PATH.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.git_program, None);
    let path = crate::paths::temp_dir().join("tools").join("git").display().to_string();
    let chosen = Settings { git_program: Some(path.clone()), ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&chosen).unwrap()).unwrap();
    assert_eq!(back.git_program, Some(path));
}

#[test]
fn git_banner_hidden_for_defaults_to_none() {
    // Files written before the setting existed show the too-old banner.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.git_banner_hidden_for, None);
    let hidden = Settings { git_banner_hidden_for: Some("2.34.1".into()), ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&hidden).unwrap()).unwrap();
    assert_eq!(back.git_banner_hidden_for.as_deref(), Some("2.34.1"));
}

#[test]
fn remember_https_credentials_default_on() {
    // Files written before the setting existed remember HTTPS sign-ins.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.remember_https_credentials);
    assert!(Settings::default().remember_https_credentials);
    let off = Settings { remember_https_credentials: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.remember_https_credentials);
}

#[test]
fn gpg_prompts_in_app_default_on() {
    // Files written before the setting existed ask for signing passphrases in Glowfly.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.gpg_prompts_in_app);
    assert!(Settings::default().gpg_prompts_in_app);
    let off = Settings { gpg_prompts_in_app: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.gpg_prompts_in_app);
}

#[test]
fn extended_logging_defaults_off_and_old_files_load() {
    // Files written before the setting existed load with it off.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.extended_logging);
    assert!(!Settings::default().extended_logging);
    let on = Settings { extended_logging: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.extended_logging);
}

#[test]
fn menu_bar_defaults_shown_and_old_files_load() {
    // Files written before the setting existed show the menu bar.
    assert!(Settings::default().show_menu_bar);
    let empty: Settings = serde_json::from_str("{}").unwrap();
    assert!(empty.show_menu_bar);
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5, "show_toolbar": false}"#).unwrap();
    assert!(s.show_menu_bar);
    let off = Settings { show_menu_bar: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.show_menu_bar);
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
fn conflict_check_defaults_on() {
    assert!(Settings::default().conflict_check);
    // Files written before the setting existed check for conflicts.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.conflict_check);
    let off = Settings { conflict_check: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.conflict_check);
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
fn word_wrap_defaults_off() {
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.diff_word_wrap);
    let on = Settings { diff_word_wrap: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.diff_word_wrap);
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
fn detached_head_notice_defaults_unshown() {
    assert!(!Settings::default().detached_head_warned);
    // Files written before the notice existed show it once.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.detached_head_warned);
    let shown = Settings { detached_head_warned: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&shown).unwrap()).unwrap();
    assert!(back.detached_head_warned);
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

#[test]
fn auto_stash_default_off() {
    assert!(!Settings::default().auto_stash);
    // Files written before the preference existed ask before stashing.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.auto_stash);
    let on = Settings { auto_stash: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.auto_stash);
}

#[test]
fn favorite_repos_default_empty_and_round_trip() {
    assert!(Settings::default().favorite_repos.is_empty());
    // Files written before favorites existed load with none.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.favorite_repos.is_empty());
    let favs = Settings { favorite_repos: vec!["C:/work/web-app".into(), "C:/work/api".into()], ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&favs).unwrap()).unwrap();
    assert_eq!(back.favorite_repos, ["C:/work/web-app", "C:/work/api"]);
}

#[test]
fn project_folders_default_empty_and_round_trip() {
    assert!(Settings::default().project_folders.is_empty());
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.project_folders.is_empty(), "files written before project folders existed load with none");
    let dirs = Settings { project_folders: vec!["/home/me/src".into(), "/home/me/work".into()], ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&dirs).unwrap()).unwrap();
    assert_eq!(back.project_folders, ["/home/me/src", "/home/me/work"]);
}

pub(super) fn json(s: &Settings) -> serde_json::Value {
    serde_json::to_value(s).unwrap()
}

pub(super) fn scratch_settings(name: &str, text: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = crate::paths::temp_dir().join(format!("glowfly-settings-{name}-{}", std::process::id()));
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
    assert_eq!(json(&s), json(&Settings { welcome_done: true, ..Settings::default() }));
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
    assert_eq!(json(&s), json(&Settings { welcome_done: true, ..Settings::default() }));
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
    let dir = crate::paths::temp_dir().join(format!("glowfly-settings-missing-{}", std::process::id()));
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

#[test]
fn trust_decisions_default_empty_and_round_trip() {
    // Files written before trust decisions existed load with none.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.repo_trust.is_empty());
    let mut s = Settings::default();
    s.repo_trust.insert("C:/a".into(), RepoTrust::Trusted);
    s.repo_trust.insert("C:/b".into(), RepoTrust::NotTrusted);
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.repo_trust, s.repo_trust);
}

#[test]
fn trust_prints_round_trip_and_follow_the_decision() {
    use crate::settings::SavedTrust;
    use crate::trust::TrustPrint;
    // Files written before prints were kept load with none.
    let s: Settings = serde_json::from_str(r#"{"repo_trust": {"/w/a": "Trusted"}}"#).unwrap();
    assert!(s.repo_trust_prints.is_empty());
    assert_eq!(s.saved_trust(std::path::Path::new("/w/a")), Some(SavedTrust::trusted(None)));

    let mut s = Settings::default();
    let real = crate::paths::temp_dir().join("glowfly-print-key");
    let print = TrustPrint(["0123456789abcdef".to_string()].into());
    s.set_trust(&real, Some(SavedTrust::trusted(Some(print.clone()))));
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.repo_trust_prints, s.repo_trust_prints);
    assert_eq!(back.saved_trust(&real), Some(SavedTrust::trusted(Some(print))));
    // Only the hashes are saved.
    assert!(serde_json::to_string(&s.repo_trust_prints).unwrap().contains(r#"["0123456789abcdef"]"#));

    s.set_trust(&real, Some(SavedTrust { trust: RepoTrust::NotTrusted, print: None }));
    assert!(s.repo_trust_prints.is_empty(), "Don't trust drops the print");
    assert_eq!(s.saved_trust(&real).map(|t| t.trust), Some(RepoTrust::NotTrusted));
    s.set_trust(&real, None);
    assert!(s.repo_trust.is_empty() && s.saved_trust(&real).is_none());
}

#[test]
fn column_widths_old_files() {
    // Files written before columns could be resized keep the old widths.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.column_widths, ColumnWidths::default());
    // A partial object fills in the rest.
    let s: Settings = serde_json::from_str(r#"{"column_widths":{"author":200}}"#).unwrap();
    assert_eq!(s.column_widths.get(ResizableColumn::Commit), 200.0);
    assert_eq!(s.column_widths, ColumnWidths { commit: 200.0, ..ColumnWidths::default() });
    let wide = Settings { column_widths: ColumnWidths::default().with(ResizableColumn::Refs, 300.0), ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&wide).unwrap()).unwrap();
    assert_eq!(back.column_widths, wide.column_widths);
}

#[test]
fn behind_alerts_default_on() {
    assert!(Settings::default().behind_alerts);
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.behind_alerts, "old files load with behind alerts on");
    let off = Settings { behind_alerts: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.behind_alerts);
}

#[test]
fn desktop_notifications_default_off() {
    assert!(!Settings::default().desktop_notifications);
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(!s.desktop_notifications, "old files load with desktop notifications off");
    let on = Settings { desktop_notifications: true, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
    assert!(back.desktop_notifications);
}

#[test]
fn keybindings_default_and_roundtrip() {
    // Files written before shortcuts could be changed load with every key at its default.
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(s.keybindings.is_empty());
    assert!(Settings::default().keybindings.is_empty());
    let mut keys = BTreeMap::new();
    keys.insert("refresh".to_owned(), vec!["Mod+R".to_owned(), "F5".to_owned()]);
    keys.insert("new-tab".to_owned(), Vec::new());
    let s = Settings { keybindings: keys.clone(), ..Settings::default() };
    let text = serde_json::to_string(&s).unwrap();
    assert!(text.find("\"new-tab\"") < text.find("\"refresh\""), "written in key order: {text}");
    let back: Settings = serde_json::from_str(&text).unwrap();
    assert_eq!(back.keybindings, keys);
}

#[test]
fn theme_choices_old_and_new() {
    for (text, theme) in [("Dark", ThemeChoice::Graphite), ("Light", ThemeChoice::Light), ("System", ThemeChoice::System)] {
        let s: Settings = serde_json::from_str(&format!(r#"{{"theme": "{text}"}}"#)).unwrap();
        assert_eq!(s.theme, theme, "files written before custom themes");
    }
    for theme in [ThemeChoice::Custom("ocean".into()), ThemeChoice::HighContrastLight] {
        let (dir, file) = scratch_settings("theme-roundtrip", b"{}");
        Settings { theme: theme.clone(), ..Settings::default() }.save_to(&file).unwrap();
        let (back, problem) = Settings::load_from(&file);
        assert_eq!((back.theme, problem), (theme, None));
        let _ = std::fs::remove_dir_all(&dir);
    }
    let custom = serde_json::to_value(ThemeChoice::Custom("ocean".into())).unwrap();
    assert_eq!(custom, serde_json::json!({"Custom": "ocean"}));
}

#[test]
fn unreadable_theme_resets_only_theme() {
    for text in [&br#"{"theme": {"Custom": 5}, "tab_size": 3}"#[..], br#"{"theme": "Neon", "tab_size": 3}"#] {
        let (dir, file) = scratch_settings("theme-bad", text);
        let (s, problem) = Settings::load_from(&file);
        assert_eq!((s.theme, s.tab_size), (ThemeChoice::Graphite, 3));
        assert_eq!(problem.unwrap().reset, Some(vec!["theme".to_owned()]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn old_theme_choices_load_as_the_default_pair() {
    // "Dark" was always the default dark theme (green, then Dusk), so it opens in today's default,
    // Graphite; "Light" stays Dawn. The Garden pair has its own names.
    let s: Settings = serde_json::from_str(r#"{"theme": "Dark"}"#).unwrap();
    assert_eq!(s.theme, ThemeChoice::Graphite);
    let s: Settings = serde_json::from_str(r#"{"theme": "Light"}"#).unwrap();
    assert_eq!(s.theme, ThemeChoice::Light);
    let json = serde_json::to_string(&ThemeChoice::GardenDark).unwrap();
    assert_eq!(json, r#""GardenDark""#);
    assert_eq!(serde_json::from_str::<ThemeChoice>(&json).unwrap(), ThemeChoice::GardenDark);
}

#[test]
fn theme_choice_builtin_mapping() {
    use super::themes::BuiltinTheme as B;
    assert_eq!(ThemeChoice::System.builtin(true), Some(B::Graphite));
    assert_eq!(ThemeChoice::System.builtin(false), Some(B::Light));
    assert_eq!(ThemeChoice::Dark.builtin(false), Some(B::Dark));
    assert_eq!(ThemeChoice::Light.builtin(true), Some(B::Light));
    assert_eq!(ThemeChoice::HighContrastDark.builtin(false), Some(B::HighContrastDark));
    assert_eq!(ThemeChoice::HighContrastLight.builtin(true), Some(B::HighContrastLight));
    assert_eq!(ThemeChoice::Sand.builtin(true), Some(B::Sand));
    assert_eq!(
        (ThemeChoice::GardenDark.builtin(false), ThemeChoice::GardenLight.builtin(true)),
        (Some(B::GardenDark), Some(B::GardenLight))
    );
    assert!(B::GardenDark.is_dark() && !B::GardenLight.is_dark());
    assert_eq!(ThemeChoice::Custom("ocean".into()).builtin(true), None);
}

#[test]
fn zoom_and_panels_default_for_old_files() {
    // Files written before zoom and panel visibility existed open at 100% with every panel shown
    // (a per-field #[serde(default)] on the bools would make them false).
    let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert_eq!(s.zoom_percent, 100);
    assert!(s.show_left_panel && s.show_details_panel && s.show_toolbar);
    let s: Settings = serde_json::from_str(r#"{"zoom_percent": 1000}"#).unwrap();
    assert_eq!(s.zoom_level(), 200);
    let off = Settings { show_toolbar: false, ..Settings::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.show_toolbar && back.show_left_panel);
}

/// The graph keys as the version before the COMMIT column wrote them.
const OLD_GRAPH_KEYS: &str = r#""columns": {"refs": true, "tree": true, "sha": true, "message": true,
    "description": "OnHover", "changes": false, "author": false, "date": true},
    "column_widths": {"refs": 200.0, "sha": 90.0, "changes": 100.0, "author": 210.0, "date": 120.0}"#;

#[test]
fn old_graph_columns_load_without_resets() {
    let text = format!(r#"{{"auto_fetch_minutes": 7, {OLD_GRAPH_KEYS}}}"#);
    let (dir, file) = scratch_settings("old-columns", text.as_bytes());
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(problem, None);
    assert!(s.columns.commit && s.columns.commit_sha, "the old SHA column shows the commit column");
    assert_eq!(s.columns.description, DescriptionMode::OnHover);
    assert_eq!((s.column_widths.get(ResizableColumn::Commit), s.column_widths.get(ResizableColumn::Refs)), (210.0, 200.0));
    let _ = std::fs::remove_dir_all(&dir);

    // Through the lenient path (another key is unreadable), the graph keys are still kept.
    let text = format!(r#"{{"theme": "Sepia", {OLD_GRAPH_KEYS}}}"#);
    let (dir, file) = scratch_settings("old-columns-lenient", text.as_bytes());
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(problem.and_then(|p| p.reset), Some(vec!["theme".to_owned()]));
    assert!(s.columns.commit);
    assert_eq!(s.column_widths.get(ResizableColumn::Commit), 210.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn graphite_is_the_default_dark_theme_and_dusk_keeps_its_own_name() {
    use super::themes::BuiltinTheme as B;
    assert_eq!(Settings::default().theme, ThemeChoice::Graphite);
    assert_eq!((B::DEFAULT_DARK, B::DEFAULT_LIGHT), (B::Graphite, B::Light));
    assert_eq!(ThemeChoice::BUILT_IN[0], ThemeChoice::Graphite, "the default dark theme leads the menus");
    // Dusk, picked on purpose, saves under its own name and comes back as Dusk.
    let json = serde_json::to_string(&ThemeChoice::Dark).unwrap();
    assert_eq!(json, r#""Dusk""#);
    assert_eq!(serde_json::from_str::<ThemeChoice>(&json).unwrap(), ThemeChoice::Dark);
    assert_eq!(serde_json::to_string(&ThemeChoice::Graphite).unwrap(), r#""Graphite""#);
    let cat = super::ThemeCatalog::default();
    assert_eq!(cat.base_of(&ThemeChoice::Custom("gone".into()), true), B::Graphite, "a missing file shows the default");
}
