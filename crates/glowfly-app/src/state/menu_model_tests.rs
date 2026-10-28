//! The menu model: each menu's rows in order, the keys and reasons its items show, their marks
//! and actions, the language, the Open recent rows, and the item numbering both the drawn and
//! the system menus rely on.

use super::*;
use crate::keymap::{BindingId as I, Chord, KeyName};
use crate::msg::{AppMsg, PrefsMsg, RecentMsg, SearchMsg};
use crate::state::menu_bar::{shas_text, tab_chord};
use crate::state::selection::MultiSelection;
use crate::state::updates::UpdateStatus;
use crate::state::window_chrome::Chrome;
use crate::state::{PrefPage, Selection, Tab, op_fixture};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{DiffLayout, DiffMode, Settings, ThemeChoice};
use std::path::PathBuf;
use std::time::Instant;

pub(crate) fn state() -> AppState {
    AppState::new(Settings::default(), Vec::new(), Instant::now())
}

/// A state with the one-commit fixture loaded.
fn with_repo(name: &str) -> AppState {
    let mut st = state();
    st.snap = Some(op_fixture::snapshot(name));
    st
}

/// The rows as text: labels, "---", "[heading]" and "submenu >".
pub(crate) fn rows(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|e| match e {
            Entry::Item(it) => Some(it.label.clone()),
            Entry::Separator => Some("---".into()),
            Entry::Heading(h) => Some(format!("[{h}]")),
            Entry::Submenu(s) => Some(format!("{} >", s.label)),
            Entry::Taken => None,
        })
        .collect()
}

/// The item labelled `label`, in submenus too.
pub(crate) fn find<'a>(entries: &'a [Entry], label: &str) -> &'a Item {
    fn walk<'a>(entries: &'a [Entry], label: &str) -> Option<&'a Item> {
        entries.iter().find_map(|e| match e {
            Entry::Item(it) if it.label == label => Some(it),
            Entry::Submenu(s) => walk(&s.entries, label),
            _ => None,
        })
    }
    walk(entries, label).unwrap_or_else(|| panic!("no item {label}"))
}

fn submenu(entries: &[Entry], kind: SubmenuKind) -> &Submenu {
    entries.iter().find_map(|e| if let Entry::Submenu(s) = e { (s.kind == kind).then_some(s) } else { None }).expect("submenu")
}

fn sent(it: &Item) -> &Msg {
    match &it.action {
        MenuAction::Send(m) => m,
        _ => panic!("{} sends no message", it.label),
    }
}

#[test]
fn file_menu_lists_its_items_in_order() {
    let st = state();
    let want = [
        "New tab",
        "Open repository…",
        "Clone repository…",
        "New repository…",
        "Open recent >",
        "---",
        "Reopen closed tab",
        "Close tab",
        "---",
        "Preferences…",
        "---",
        "Quit",
    ];
    assert_eq!(rows(&entries(&st, TopMenu::File)), want);
}

#[test]
fn edit_window_and_help_list_their_items_in_order() {
    let st = state();
    let edit = rows(&entries(&st, TopMenu::Edit));
    assert_eq!(
        &edit[2..],
        ["---", "Find commits…", "Show only matching commits", "---", "Copy commit SHA", "Copy commit message", "Copy branch name"]
    );
    assert!(edit[0].starts_with("Undo") && edit[1].starts_with("Redo"), "{edit:?}");
    let window = ["New tab", "---", "Next tab", "Previous tab", "Close other tabs", "Close tabs to the right"];
    assert_eq!(rows(&entries(&st, TopMenu::Window)), window);
    let help = [
        "Keyboard shortcuts",
        "Command palette",
        "---",
        "Show the welcome",
        "Tour of the main window",
        "---",
        "Activity log",
        "Error log",
        "Performance log",
        "Open logs folder",
        "Export logs…",
        "---",
        "Glowfly website",
        "Report an issue",
        "---",
        "Release notes",
        "Check for updates",
        "About Glowfly…",
    ];
    assert_eq!(rows(&entries(&st, TopMenu::Help)), help);
}

#[test]
fn view_menu_lists_its_items_in_order() {
    let st = state();
    assert_eq!(st.view.chrome, Chrome::System);
    let want = [
        "Zoom in",
        "Zoom out",
        "Reset zoom to 100%",
        "---",
        "Left panel",
        "Commit details panel",
        "Toolbar",
        "Menu bar",
        "Full screen",
        "Terminal",
        "Activity log",
        "Operation journal",
        "---",
        "[Diff layout]",
        "Inline",
        "Side by side",
        "[Diff shows]",
        "Changed lines only",
        "Whole file",
        "Ignore whitespace",
        "Wrap long lines",
        "Theme >",
        "---",
        "Refresh",
    ];
    assert_eq!(rows(&entries(&st, TopMenu::View)), want);
    let theme = rows(&submenu(&entries(&st, TopMenu::View), SubmenuKind::Theme).entries);
    assert_eq!(theme.last().map(String::as_str), Some("Theme settings…"));
    assert_eq!(theme[theme.len() - 2], "---");
}

#[test]
fn keyed_items_show_the_keymaps_first_key() {
    let mut st = state();
    let menus = build(&st);
    let keyed: Vec<&Item> = items(&menus).into_iter().filter(|it| it.key.is_some() && !matches!(it.mark, Mark::Selected(_))).collect();
    assert!(keyed.len() > 15, "{}", keyed.len());
    for it in keyed {
        let id = it.key.unwrap();
        assert_eq!(it.chord, st.keymap.chords(id).first().copied(), "{}", it.label);
        assert_eq!(it.chord.map(|c| c.text()).as_deref(), st.keymap.hint(id), "{}", it.label);
    }
    let quit_key = Chord::cmd_shift(KeyName::Letter('Q'));
    st.keymap.set(I::Quit, vec![quit_key]);
    st.keymap.set(I::Refresh, Vec::new());
    assert_eq!(find(&entries(&st, TopMenu::File), "Quit").chord, Some(quit_key));
    assert_eq!(find(&entries(&st, TopMenu::View), "Refresh").chord, None);
}

#[test]
fn blocked_reasons_match_the_drawn_menus() {
    let mut st = state();
    let t = Lang::En;
    let no_repo = Some(t.tr("menu.common.no_repo").to_owned());
    let edit = entries(&st, TopMenu::Edit);
    assert_eq!(find(&edit, "Find commits…").blocked, no_repo);
    assert_eq!(find(&edit, "Show only matching commits").blocked, no_repo);
    let view = entries(&st, TopMenu::View);
    assert_eq!(find(&view, "Terminal").blocked, no_repo);
    assert_eq!(find(&view, "Operation journal").blocked, no_repo);
    let window = entries(&st, TopMenu::Window);
    for label in ["Next tab", "Previous tab"] {
        assert_eq!(find(&window, label).blocked.as_deref(), Some(t.tr("menu.window.one_tab")));
    }
    assert_eq!(find(&window, "Close other tabs").blocked.as_deref(), Some(t.tr("menu.window.no_others")));
    assert_eq!(find(&window, "Close tabs to the right").blocked.as_deref(), Some(t.tr("menu.window.none_right")));
    let file = entries(&st, TopMenu::File);
    assert_eq!(find(&file, "Reopen closed tab").blocked.as_deref(), Some(t.tr("menu.file.no_closed")));
    assert_eq!(submenu(&file, SubmenuKind::Recent).blocked.as_deref(), Some(t.tr("menu.file.no_recent")));
    st.settings.zoom_percent = glowfly_core::settings::ZOOM_STEPS[glowfly_core::settings::ZOOM_STEPS.len() - 1];
    assert!(find(&entries(&st, TopMenu::View), "Zoom in").blocked.is_some());
    assert!(find(&entries(&st, TopMenu::Help), "Check for updates").blocked.is_none());
    st.updates.status = UpdateStatus::Checking { manual: true, previous: Box::new(UpdateStatus::Idle) };
    assert_eq!(find(&entries(&st, TopMenu::Help), "Check for updates").blocked.as_deref(), Some(t.tr("menu.help.busy")));
    st.recent = vec![glowfly_core::paths::temp_dir().join("glowfly-menu-model")];
    assert!(submenu(&entries(&st, TopMenu::File), SubmenuKind::Recent).blocked.is_none());
}

#[test]
fn marks_follow_the_state() {
    let mut st = state();
    st.settings.show_left_panel = false;
    st.settings.show_toolbar = true;
    st.view.window.fullscreen = true;
    st.view.show_log = true;
    st.settings.diff_layout = DiffLayout::Split;
    st.settings.diff_mode = DiffMode::WholeFile;
    st.settings.diff_ignore_whitespace = true;
    st.settings.diff_word_wrap = false;
    st.settings.theme = ThemeChoice::Light;
    let view = entries(&st, TopMenu::View);
    let mark = |label| find(&view, label).mark;
    assert_eq!(mark("Left panel"), Mark::Check(false));
    assert_eq!(mark("Toolbar"), Mark::Check(true));
    assert_eq!(mark("Full screen"), Mark::Check(true));
    assert_eq!(mark("Activity log"), Mark::Check(true));
    assert_eq!(mark("Inline"), Mark::Radio(false));
    assert_eq!(mark("Side by side"), Mark::Radio(true));
    assert_eq!(mark("Whole file"), Mark::Radio(true));
    assert_eq!(mark("Ignore whitespace"), Mark::Toggle(true));
    assert_eq!(mark("Wrap long lines"), Mark::Toggle(false));
    assert_eq!(mark("Dawn"), Mark::Radio(true));
    assert_eq!(mark("Dusk"), Mark::Radio(false));
    let dir = |n: &str| glowfly_core::paths::temp_dir().join(n);
    st.tabs = vec![Tab::unloaded(dir("a")), Tab::unloaded(dir("b")), Tab::unloaded(dir("c"))];
    st.active_tab = 1;
    let window = entries(&st, TopMenu::Window);
    let tabs: Vec<&Item> = window.iter().filter_map(|e| if let Entry::Item(it) = e { Some(it) } else { None }).take(3).collect();
    for (i, it) in tabs.iter().enumerate() {
        assert_eq!(it.mark, Mark::Selected(i == 1), "{}", it.label);
        assert_eq!(it.chord, tab_chord(&st.keymap, i, 3));
        assert_eq!(it.chord.map(|c| c.key), Some(KeyName::Digit(i as u8 + 1)));
        assert!(matches!(sent(it), Msg::Tabs(crate::msg::TabsMsg::SelectTab(n)) if *n == i));
    }
}

#[test]
fn drawn_title_bars_have_no_menu_bar_item() {
    let mut st = state();
    for (chrome, has) in [(Chrome::MacOs, false), (Chrome::Windows, false), (Chrome::System, true)] {
        st.view.chrome = chrome;
        assert_eq!(rows(&entries(&st, TopMenu::View)).iter().any(|r| r == "Menu bar"), has, "{chrome:?}");
    }
}

#[test]
fn actions_do_what_the_menus_did() {
    let mut st = with_repo("menu-model-actions");
    let mut msgs = Vec::new();
    MenuAction::ToggleActivityLog.run(&mut st, &mut msgs);
    assert!(st.view.show_log && msgs.is_empty());
    MenuAction::ToggleActivityLog.run(&mut st, &mut msgs);
    assert!(!st.view.show_log);
    MenuAction::ShowFilter.run(&mut st, &mut msgs);
    assert!(st.view.show_search);
    assert!(matches!(&msgs[..], [Msg::Search(SearchMsg::ToggleFilter)]));
    let file = entries(&st, TopMenu::File);
    let quit = find(&file, "Quit");
    assert_eq!(quit.role, Role::Quit);
    assert!(matches!(sent(quit), Msg::App(AppMsg::Quit)));
    let prefs = find(&file, "Preferences…");
    assert_eq!(prefs.role, Role::Prefs);
    assert!(matches!(sent(prefs), Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General))));
    let help = entries(&st, TopMenu::Help);
    let about = find(&help, "About Glowfly…");
    assert_eq!(about.role, Role::About);
    assert!(matches!(sent(about), Msg::App(AppMsg::ShowAbout)));
    let edit = entries(&st, TopMenu::Edit);
    assert!(matches!(find(&edit, "Show only matching commits").action, MenuAction::ShowFilter));
    assert!(matches!(find(&entries(&st, TopMenu::View), "Activity log").action, MenuAction::ToggleActivityLog));
    let head = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    st.session.selection = Selection::Commit(head);
    let edit = entries(&st, TopMenu::Edit);
    assert!(matches!(sent(find(&edit, "Copy commit SHA")), Msg::Copy(s) if *s == head.to_string()));
    let other = git2::Oid::from_str("1234567890123456789012345678901234567890").unwrap();
    let multi = MultiSelection {
        oids: vec![head, other],
        anchor: head,
        cursor: head,
        members: [head, other].into(),
        span: crate::state::selection_span::Span::Pair { older: other, newer: head },
        swapped: false,
        changes: None,
        stacked: None,
        pick_blocked: None,
        export_blocked: None,
    };
    let want = shas_text(&multi);
    assert_eq!(want, format!("{head}\n{other}"));
    st.session.selection = Selection::Commits(multi);
    let edit = entries(&st, TopMenu::Edit);
    assert!(matches!(sent(find(&edit, "Copy 2 commit SHAs")), Msg::Copy(s) if *s == want));
}

#[test]
fn labels_follow_the_language() {
    let mut st = state();
    st.settings.language = "de".into();
    let menus = build(&st);
    for (m, menu) in TopMenu::ALL.into_iter().zip(&menus) {
        assert_eq!(menu.title, m.title(Lang::De));
    }
    assert_ne!(menus[0].title, "File");
    let quit = Lang::De.tr("menu.file.quit");
    assert_ne!(quit, "Quit");
    assert_eq!(find(&menus[0].entries, quit).role, Role::Quit);
}

#[test]
fn open_recent_lists_favorites_then_recent_then_clear() {
    let dir = |n: &str| glowfly_core::paths::temp_dir().join("glowfly-menu-recent").join(n);
    let (fav, a, b) = (dir("fav"), dir("a"), dir("b"));
    let settings = Settings { favorite_repos: vec![glowfly_core::paths::encode(&fav)], ..Settings::default() };
    let st = AppState::new(settings, vec![a.clone(), fav.clone(), b.clone()], Instant::now());
    let file = entries(&st, TopMenu::File);
    let recent = submenu(&file, SubmenuKind::Recent);
    let shown = |p: &PathBuf| glowfly_core::native_path::shown(p);
    let want = [
        "[Favorites]".to_owned(),
        shown(&fav),
        "---".into(),
        "[Recent]".into(),
        shown(&a),
        shown(&b),
        "---".into(),
        "Clear recent…".into(),
    ];
    assert_eq!(rows(&recent.entries), want);
    assert!(matches!(sent(find(&recent.entries, &shown(&a))), Msg::Repo(crate::msg::RepoMsg::Open(p)) if *p == a));
    let clear = sent(find(&recent.entries, "Clear recent…"));
    assert!(matches!(clear, Msg::Confirm { then, .. } if matches!(**then, Msg::Recent(RecentMsg::ClearRecent))));
}

#[test]
fn walk_and_take_agree() {
    let mut st = state();
    st.recent = vec![glowfly_core::paths::temp_dir().join("glowfly-menu-walk")];
    let mut menus = build(&st);
    let labels: Vec<String> = items(&menus).iter().map(|it| it.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "Theme settings…") && labels.iter().any(|l| l == "Clear recent…"));
    for (k, label) in labels.iter().enumerate() {
        assert_eq!(take_item(&mut menus, k).map(|it| it.label).as_deref(), Some(label.as_str()), "item {k}");
    }
    assert!(take_item(&mut menus, 0).is_none(), "taken once");
    assert!(take_item(&mut menus, labels.len()).is_none(), "past the end");
}
