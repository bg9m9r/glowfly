//! The system menu bar's plan: key equivalents, the app menu and what it takes from the others,
//! tidy separators, marks and tips, when the menus must be built anew, and which picks count.

use super::*;
use crate::keymap::{BindingId as I, Chord, KeyName as K};
use crate::msg::{AppMsg, Msg};
use crate::state::AppState;
use crate::state::menu_model::tests::state;
use crate::state::menu_model::{Item, build, items};
use crate::state::{Tab, menu_keys::TopMenu};
use glowfly_core::settings::ThemeFile;

fn key(k: &str, command: bool, shift: bool, alt: bool, ctrl: bool) -> Option<KeyEquivalent> {
    Some(KeyEquivalent { key: k.into(), command, shift, alt, ctrl })
}

fn plain(k: &str) -> Option<KeyEquivalent> {
    key(k, false, false, false, false)
}

fn title(e: &NEntry) -> String {
    match e {
        NEntry::Item { title, .. } | NEntry::Standard { title, .. } | NEntry::Services(title) | NEntry::Heading(title) => title.clone(),
        NEntry::Submenu { title, .. } => format!("{title} >"),
        NEntry::Separator => "---".into(),
    }
}

fn titles(entries: &[NEntry]) -> Vec<String> {
    entries.iter().map(title).collect()
}

fn find<'a>(entries: &'a [NEntry], name: &str) -> &'a NEntry {
    entries.iter().find(|e| title(e) == name).unwrap_or_else(|| panic!("no {name}"))
}

/// The index of the item labelled `label` in the model's numbering.
fn index_of(st: &AppState, label: &str) -> usize {
    let menus = build(st);
    items(&menus).iter().position(|it: &&Item| it.label == label).expect("item")
}

#[test]
fn key_equivalents_follow_the_chord() {
    let ke = |c: Chord| key_equivalent(c);
    assert_eq!(ke(Chord::cmd(K::Letter('Q'))), key("q", true, false, false, false));
    assert_eq!(ke(Chord::cmd_shift(K::Letter('P'))), key("p", true, true, false, false));
    assert_eq!(ke(Chord::cmd_ctrl(K::Letter('F'))), key("f", true, false, false, true));
    assert_eq!(ke(Chord::alt(K::Letter('X'))), key("x", false, false, true, false));
    assert_eq!(ke(Chord::plain(K::F(5))), plain("\u{F708}"));
    assert_eq!(ke(Chord::plain(K::F(1))), plain("\u{F704}"));
    assert_eq!(ke(Chord::plain(K::F(12))), plain("\u{F70F}"));
    for (k, want) in [
        (K::Up, "\u{F700}"),
        (K::Down, "\u{F701}"),
        (K::Left, "\u{F702}"),
        (K::Right, "\u{F703}"),
        (K::Home, "\u{F729}"),
        (K::End, "\u{F72B}"),
        (K::PageUp, "\u{F72C}"),
        (K::PageDown, "\u{F72D}"),
        (K::Insert, "\u{F727}"),
        (K::Delete, "\u{F728}"),
        (K::Backspace, "\u{8}"),
        (K::Enter, "\r"),
        (K::Escape, "\u{1b}"),
        (K::Tab, "\t"),
        (K::Space, " "),
        (K::Comma, ","),
        (K::Equals, "="),
        (K::Plus, "+"),
        (K::Slash, "/"),
        (K::Digit(3), "3"),
    ] {
        assert_eq!(ke(Chord::plain(k)), plain(want), "{k:?}");
    }
    assert_eq!(ke(Chord::cmd(K::DigitRange(1, 8))), None);
}

#[test]
fn the_app_menu_takes_about_preferences_and_quit() {
    let mut st = state();
    let p = plan(&build(&st), Lang::En, 0);
    let want = [
        "About Glowfly",
        "---",
        "Preferences…",
        "---",
        "Services",
        "---",
        "Hide Glowfly",
        "Hide Others",
        "Show All",
        "---",
        "Quit Glowfly",
    ];
    assert_eq!(titles(&p.app), want);
    let NEntry::Item { tag: quit, key: quit_key, .. } = find(&p.app, "Quit Glowfly") else { panic!("Quit is an item") };
    assert_eq!(untag(*quit), (0, index_of(&st, "Quit")));
    assert_eq!(*quit_key, key_equivalent(st.keymap.chords(I::Quit)[0]));
    assert_eq!(
        find(&p.app, "Hide Glowfly"),
        &NEntry::Standard { title: "Hide Glowfly".into(), key: key("h", true, false, false, false), action: Standard::Hide }
    );
    let others = NEntry::Standard { title: "Hide Others".into(), key: key("h", true, false, true, false), action: Standard::HideOthers };
    assert_eq!(find(&p.app, "Hide Others"), &others);
    assert_eq!(find(&p.app, "Show All"), &NEntry::Standard { title: "Show All".into(), key: None, action: Standard::ShowAll });
    let file = titles(&p.menus[0].entries);
    assert_eq!(file.last().map(String::as_str), Some("Close tab"), "{file:?}");
    assert!(!file.iter().any(|t| t == "Preferences…" || t == "Quit"));
    let help = titles(&p.menus[4].entries);
    assert!(!help.iter().any(|t| t.starts_with("About")), "{help:?}");
    assert_eq!(help.last().map(String::as_str), Some("Check for updates"));
    // A rebound Quit key shows in the app menu.
    st.keymap.set(I::Quit, vec![Chord::cmd_shift(K::Letter('Q'))]);
    let p = plan(&build(&st), Lang::En, 0);
    let NEntry::Item { key: k, .. } = find(&p.app, "Quit Glowfly") else { panic!() };
    assert_eq!(*k, key("q", true, true, false, false));
}

#[test]
fn tidy_drops_dangling_separators() {
    let h = |s: &str| NEntry::Heading(s.into());
    let s = || NEntry::Separator;
    assert_eq!(tidy(vec![s(), h("a"), s(), s(), h("b"), s()]), vec![h("a"), s(), h("b")]);
    assert_eq!(tidy(vec![s(), s()]), vec![]);
    assert_eq!(tidy(vec![h("a"), s(), h("b")]), vec![h("a"), s(), h("b")]);
}

#[test]
fn plan_marks_and_tips() {
    let mut st = state();
    st.view.show_log = true;
    let p = plan(&build(&st), Lang::En, 3);
    let view = &p.menus[2].entries;
    let NEntry::Item { on, enabled, tip, .. } = find(view, "Activity log") else { panic!() };
    assert!(*on && *enabled && tip.is_none());
    let NEntry::Item { on, .. } = find(view, "Inline") else { panic!() };
    assert!(*on, "the default layout's radio");
    let NEntry::Item { enabled, tip, .. } = find(view, "Terminal") else { panic!() };
    assert!(!enabled);
    assert_eq!(tip.as_deref(), Some(Lang::En.tr("menu.common.no_repo")));
    let window = &p.menus[3].entries;
    let NEntry::Item { on, .. } = &window[0] else { panic!() };
    assert!(*on, "the active tab");
    assert_eq!(find(view, "Diff layout"), &NEntry::Heading("Diff layout".into()));
    let recent = find(&p.menus[0].entries, "Open recent >");
    let no_recent = Some(Lang::En.tr("menu.file.no_recent").to_owned());
    assert_eq!(recent, &NEntry::Submenu { title: "Open recent".into(), enabled: false, tip: no_recent, entries: vec![] });
}

#[test]
fn shape_changes_only_with_structure() {
    let mut st = state();
    let first = plan(&build(&st), Lang::En, 1);
    let same = |st: &AppState| same_shape(&first, &plan(&build(st), Lang::En, 2));
    st.keymap.set(I::Refresh, Vec::new());
    st.settings.language = "de".into();
    st.settings.zoom_percent = 200;
    st.view.show_log = true;
    assert!(same(&st));
    assert_ne!(first, plan(&build(&st), Lang::De, 1));
    let dir = |n: &str| glowfly_core::paths::temp_dir().join("glowfly-menu-shape").join(n);
    let mut two = state();
    two.tabs.push(Tab::unloaded(dir("b")));
    assert!(!same(&two), "a second tab");
    let mut recent = state();
    recent.recent = vec![dir("a")];
    assert!(!same(&recent), "a recent folder");
    let mut themed = state();
    let file = ThemeFile {
        id: "ocean".into(),
        path: dir("ocean.json"),
        name: "Ocean".into(),
        base: glowfly_core::settings::BuiltinTheme::Dark,
        colors: Default::default(),
        problems: Vec::new(),
    };
    themed.themes.catalog.files = vec![file];
    assert!(!same(&themed), "a theme file");
}

#[test]
fn picks_map_to_actions_of_the_current_generation() {
    let st = state();
    assert_eq!(untag(tag(7, 42)), (7, 42));
    assert_eq!(untag(tag(u32::MAX, 3)), (u32::MAX, 3));
    let quit = index_of(&st, "Quit");
    let mut menus = build(&st);
    let got = take_picked(&mut menus, 2, &[tag(2, quit), tag(2, quit)]);
    assert!(matches!(&got[..], [MenuAction::Send(m)] if matches!(**m, Msg::App(AppMsg::Quit))), "once");
    let mut menus = build(&st);
    assert!(take_picked(&mut menus, 2, &[tag(1, quit), tag(2, 100_000)]).is_empty(), "stale or unknown");
    let next = index_of(&st, "Next tab");
    let mut menus = build(&st);
    assert!(take_picked(&mut menus, 2, &[tag(2, next)]).is_empty(), "blocked with one tab");
}

#[test]
fn only_cmd_h_and_option_cmd_h_are_appkits() {
    assert_eq!(standard_key("h", true, false, false, false), Some(Standard::Hide));
    assert_eq!(standard_key("H", true, false, false, false), Some(Standard::Hide));
    assert_eq!(standard_key("h", true, false, true, false), Some(Standard::HideOthers));
    assert_eq!(standard_key("h", true, true, false, false), None);
    assert_eq!(standard_key("h", true, false, false, true), None);
    assert_eq!(standard_key("q", true, false, false, false), None);
    assert_eq!(standard_key("h", false, false, false, false), None);
}

#[test]
fn app_menu_is_translated() {
    let mut st = state();
    st.settings.language = "de".into();
    let p = plan(&build(&st), st.settings.lang(), 0);
    let t = titles(&p.app);
    for want in ["Glowfly beenden", "Glowfly ausblenden", "Dienste"] {
        assert!(t.iter().any(|x| x == want), "{want} in {t:?}");
    }
    assert_eq!(p.menus[0].title, TopMenu::File.title(Lang::De));
}
