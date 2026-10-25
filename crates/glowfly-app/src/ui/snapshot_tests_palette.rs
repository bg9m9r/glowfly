//! The command palette over the main window: the full list (with shortcut hints), a filtered
//! list with matched characters highlighted, the tag commands, remote branch and tag checkouts,
//! the row cap, the tab commands, no matches, the file finder (its rows, loading and error
//! states), and the keys and clicks it turns into messages. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_palette_nav::palette_row;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::RepoMsg;
use crate::state::AppState;
use crate::state::palette::{Items, Mode, Palette, PaletteCommand, Pick, PickKind};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo;
use glowfly_core::settings::ThemeChoice;

fn pushed(h: &Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn palette_dark() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette");
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette");
    // An empty query groups the list under headings; the fixture suggests Stage all files.
    assert!(h.query_by_label("Suggested").is_some() && h.query_by_label("All commands").is_some());
    assert!(h.query_by_label("Recent").is_none(), "nothing run yet");
    drop(h);

    // The headings are translated.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "palette");
    st.settings.language = "de".into();
    st.palette_recent.record(PaletteCommand::Refresh.recall_key().unwrap());
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_groups_german");
    for heading in ["Zuletzt verwendet", "Vorschläge", "Alle Befehle"] {
        assert!(h.query_by_label(heading).is_some(), "{heading}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_filtered_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "palette-light");
    st.palette = Some(Palette { query: "ch".into(), selected: 1, scroll_to_selected: false, ..Default::default() });
    render("palette_filtered_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn palette_tags_and_hints() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-tags");
    st.palette = Some(Palette { query: "tag".into(), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_tags");
    assert!(h.query_by_label("Create tag at HEAD…").is_some());
    h.get_by_label("Create annotated tag at HEAD…").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CreateTag { annotated: true })))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_checkout_refs() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-refs");
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    let names = |v: &[&str]| v.iter().map(|n| (n.to_string(), oid)).collect::<Vec<_>>();
    snap.remotes = vec![("origin".into(), names(&["main", "login-page", "feature/sidebar"])), ("upstream".into(), names(&["release-2.0"]))];
    st.palette = Some(Palette { query: "checkout".into(), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_checkout_refs");
    for present in ["Checkout origin/feature/sidebar", "Checkout upstream/release-2.0", "Checkout tag v0.1.0"] {
        assert!(h.query_by_label(present).is_some(), "{present}");
    }
    // Local branches of the same name exist.
    assert!(h.query_by_label("Checkout origin/main").is_none());
    assert!(h.query_by_label("Checkout origin/login-page").is_none());
    h.get_by_label("Checkout tag v0.1.0").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CheckoutTag(t))) if t == "v0.1.0")));
    h.get_by_label("Checkout upstream/release-2.0").click();
    h.run();
    let upstream = |m: &Msg| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CheckoutRemote { remote, branch })) if remote == "upstream" && branch == "release-2.0");
    assert!(pushed(&h, upstream));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_many_refs_footer() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-many-refs");
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    snap.tags.extend((0..300).map(|i| (format!("t{i}"), oid)));
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("Showing 200 of").is_some());
    h.press_key(Key::Enter);
    h.run();
    // The first suggested command: the fixture has unstaged changes and no remote.
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::StageAll)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_more() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-more");
    st.closed_tabs = vec![std::path::PathBuf::from("C:/work/old-repo").into()];
    st.palette = Some(Palette { query: "tab".into(), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_more");
    h.get_by_label("Reopen closed tab").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::ReopenClosedTab)))));

    h.state_mut().1.clear();
    if let Some(p) = h.state_mut().0.palette.as_mut() {
        p.query = "refresh".into();
        p.selected = 0;
    }
    h.run();
    // The palette's row, and the toolbar's button under its own name.
    palette_row(&h, "Refresh");
    assert!(h.query_by_label("Refresh the repository").is_some());
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Refresh)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_no_matches() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-none");
    st.palette = Some(Palette { query: "zzzz".into(), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("No matching commands").is_some());
    // Enter with nothing listed runs nothing.
    h.press_key(Key::Enter);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(_)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_keys_and_clicks() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-keys");
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Checkout search").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Checkout(b))) if b == "search")));

    h.state_mut().1.clear();
    h.press_key(Key::ArrowDown);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::PaletteMove(1)))));
    // The graph under the palette doesn't move its selection.
    assert!(!pushed(&h, |m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_)) | Msg::Repo(RepoMsg::SelectWip))));

    h.state_mut().1.clear();
    h.press_key(Key::ArrowUp);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::PaletteMove(-1)))));

    // The first row is the first suggestion: the fixture has no remote, so nothing to pull,
    // push or fetch, and it has unstaged changes, so Stage all files.
    h.state_mut().1.clear();
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::StageAll)))));
    // A group heading isn't a row: clicking it runs nothing.
    h.state_mut().1.clear();
    h.get_by_label("All commands").click();
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(_)))));

    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::ClosePalette))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_ignores_keys_under_a_credential_prompt() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-askpass");
    st.palette = Some(Palette::default());
    let prompt = glowfly_core::askpass::Prompt::new("Password for 'https://example.com': ", None);
    st.askpass.queue.push_back(crate::state::askpass::PendingPrompt { id: 1, prompt, retry: false, rejected_before: false });
    let mut h = harness(st, true);
    h.run();
    h.press_key(Key::Enter);
    h.press_key(Key::ArrowDown);
    h.press_key(Key::Escape);
    h.run();
    assert!(!pushed(&h, |m| matches!(
        m,
        Msg::Palette(PaletteMsg::RunPaletteCommand(_)) | Msg::Palette(PaletteMsg::PaletteMove(_)) | Msg::Palette(PaletteMsg::ClosePalette)
    )));
    drop(h);
    remove_fixture(&dir);
}

/// Ctrl+P and Ctrl+Shift+P toggle the palette; nothing happens under a dialog.
#[test]
fn shortcut_toggles() {
    let st = AppState::new(super::snapshot_tests::settings(ThemeChoice::Dark), Vec::new(), std::time::Instant::now());
    let mut h = crate::ui::snapshot_check::builder()
        .build_state(|ctx, (st, msgs): &mut (AppState, Vec<Msg>)| super::shortcuts::input(ctx, st, msgs), (st, Vec::new()));
    h.press_key_modifiers(Modifiers::COMMAND, Key::P);
    h.run();
    assert_eq!(h.state().1.iter().filter(|m| matches!(m, Msg::Palette(PaletteMsg::TogglePalette))).count(), 1);
    h.state_mut().1.clear();
    h.press_key_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    h.run();
    assert_eq!(h.state().1.iter().filter(|m| matches!(m, Msg::Palette(PaletteMsg::TogglePalette))).count(), 1);
    h.state_mut().1.clear();
    h.press_key(Key::P);
    h.run();
    assert!(h.state().1.is_empty(), "P alone types");

    h.state_mut().0.prefs = Some(crate::state::PrefsState::new(crate::state::PrefPage::General));
    h.press_key_modifiers(Modifiers::COMMAND, Key::P);
    h.run();
    assert!(h.state().1.is_empty(), "not under Preferences");
}

/// With the commit search bar open and empty, typed text still lands in the palette's field
/// rather than being lost to the two fields taking focus from each other.
#[test]
fn palette_takes_typing_over_an_empty_search_bar() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-search-bar");
    st.view.show_search = true;
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    h.run();
    h.input_mut().events.push(eframe::egui::Event::Text("push".into()));
    h.run();
    assert_eq!(h.state().0.palette.as_ref().map(|p| p.query.as_str()), Some("push"));
    assert_eq!(h.state().0.view.search, "");
    drop(h);
    remove_fixture(&dir);
}

/// A palette in the file finder over `items`, with `query` typed.
fn finder(items: Items, query: &str, selected: usize) -> Palette {
    let mut pk = Pick::new(PickKind::FileHistory);
    pk.set_items(items);
    Palette { query: query.into(), selected, mode: Mode::Pick(pk), ..Default::default() }
}

fn queue_prompt(st: &mut AppState) {
    let prompt = glowfly_core::askpass::Prompt::new("Password for 'https://example.com': ", None);
    st.askpass.queue.push_back(crate::state::askpass::PendingPrompt { id: 1, prompt, retry: false, rejected_before: false });
}

#[test]
fn palette_files() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-files");
    let paths = repo::tracked_paths(&dir).unwrap();
    assert_eq!(paths, ["README.md", "app.txt", "login.txt"]);
    st.palette = Some(finder(Items::Ready(paths), "", 1));
    render("palette_files", st, true);
    remove_fixture(&dir);
}

#[test]
fn palette_files_filtered_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "palette-files-light");
    let mut paths: Vec<String> = (0..250).map(|i| format!("src/ui/panel{i}.rs")).collect();
    paths.push("docs/guide.md".into());
    st.palette = Some(finder(Items::Ready(paths), "pan", 0));
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label_contains("Showing 200 of 250 files").is_some());
    h.check_snapshot("palette_files_filtered_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_blame_finder() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-blame");
    let mut pk = Pick::new(PickKind::Blame);
    pk.set_items(Items::Ready(repo::tracked_paths(&dir).unwrap()));
    st.palette = Some(Palette { mode: Mode::Pick(pk), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("Enter to blame").is_some());
    h.check_snapshot("palette_blame_finder");
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenBlame(p))) if p == "README.md")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_file_keys() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-file-keys");
    let paths = repo::tracked_paths(&dir).unwrap();
    st.palette = Some(finder(Items::Ready(paths), "", 0));
    let mut h = harness(st, true);
    h.run();
    let opens = |want: &'static str| move |m: &Msg| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenFileHistory(p))) if p == want);
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, opens("README.md")));

    h.state_mut().1.clear();
    h.get_by_label("app.txt").click();
    h.run();
    assert!(pushed(&h, opens("app.txt")));

    h.state_mut().1.clear();
    h.press_key(Key::Backspace);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::PaletteBack))));

    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::PaletteBack))));
    assert!(!pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::ClosePalette))), "Esc goes back rather than closing");

    // Backspace with text in the field edits the text.
    h.state_mut().0.palette.as_mut().unwrap().query = "x".into();
    h.run();
    h.state_mut().1.clear();
    h.press_key(Key::Backspace);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::Palette(PaletteMsg::PaletteBack))));

    // Under a credential prompt the finder takes no keys.
    h.state_mut().0.palette.as_mut().unwrap().query.clear();
    queue_prompt(&mut h.state_mut().0);
    h.run();
    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.press_key(Key::Backspace);
    h.run();
    assert!(!pushed(&h, |m| matches!(
        m,
        Msg::Palette(PaletteMsg::PaletteBack) | Msg::Palette(PaletteMsg::ClosePalette) | Msg::Palette(PaletteMsg::RunPaletteCommand(_))
    )));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_files_loading_and_error() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-files-states");
    st.palette = Some(finder(Items::Loading, "", 0));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("Loading files").is_some());
    set_items(&mut h, Items::Failed("bad index".into()));
    h.run();
    assert!(h.query_by_label_contains("bad index").is_some());
    set_items(&mut h, Items::Ready(Vec::new()));
    h.run();
    assert!(h.query_by_label_contains("no tracked files").is_some());
    drop(h);
    remove_fixture(&dir);
}

fn set_items(h: &mut Harness<'_, (AppState, Vec<Msg>)>, items: Items) {
    if let Some(Palette { mode: Mode::Pick(pk), .. }) = h.state_mut().0.palette.as_mut() {
        pk.set_items(items);
    }
}
