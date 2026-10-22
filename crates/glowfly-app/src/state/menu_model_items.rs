//! Each menu's entries (`menu_model.rs` holds the types): the item labels from the catalog, the
//! keymap's keys, why an item is disabled and the message it sends.

use super::layout::{Panel, shown};
use super::menu_bar::{
    ISSUES, ShaCopy, WEBSITE, copy_branch, copy_message, copy_sha, shas_text, tab_binding, tab_chord, tab_name, zoom_blocked,
};
use super::menu_model::{Entry, Item, Mark, MenuAction, Role, Submenu, SubmenuKind};
use super::repo_lists::{clear_recent_msg, favorites, has_entries, recent_rows};
use super::theme_names;
use super::{AppState, PrefPage, Selection};
use crate::journal::JournalMsg;
use crate::keymap::{BindingId as I, KeyAction, Keymap};
use crate::logs::{LogKind, LogsMsg};
use crate::msg::{AppMsg, CloneMsg, DiffMsg, JobMsg, LayoutMsg, Msg, PaletteMsg, PrefsMsg, RepoMsg, SearchMsg, TabsMsg, UpdateMsg};
use crate::onboarding::OnboardingMsg;
use crate::terminal::TerminalMsg;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{DiffLayout, DiffMode, ThemeChoice};
use std::path::Path;

/// A plain item showing `key`'s first key.
fn item(keys: &Keymap, label: impl Into<String>, key: Option<I>, blocked: Option<&str>, action: MenuAction) -> Item {
    Item {
        label: label.into(),
        key,
        chord: key.and_then(|k| keys.chords(k).first().copied()),
        blocked: blocked.map(str::to_owned),
        mark: Mark::None,
        role: Role::Plain,
        action,
    }
}

/// A plain item that sends `msg`.
fn send(keys: &Keymap, label: impl Into<String>, key: Option<I>, blocked: Option<&str>, msg: Msg) -> Entry {
    Entry::Item(item(keys, label, key, blocked, MenuAction::send(msg)))
}

/// An item with `mark`.
fn marked(keys: &Keymap, label: impl Into<String>, mark: Mark, key: Option<I>, blocked: Option<&str>, action: MenuAction) -> Entry {
    Entry::Item(Item { mark, ..item(keys, label, key, blocked, action) })
}

/// An item the macOS app menu takes (`role`).
fn app_item(keys: &Keymap, label: &str, key: Option<I>, role: Role, msg: Msg) -> Entry {
    Entry::Item(Item { role, ..item(keys, label, key, None, MenuAction::send(msg)) })
}

/// File: tabs, opening, cloning and starting repositories, recent ones, Preferences and Quit.
pub(super) fn file(st: &AppState) -> Vec<Entry> {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let no_recent = (!has_entries(st)).then_some(t.tr("menu.file.no_recent"));
    let no_closed = st.closed_tabs.is_empty().then_some(t.tr("menu.file.no_closed"));
    vec![
        send(keys, t.tr("menu.file.new_tab"), Some(I::NewTab), None, Msg::Tabs(TabsMsg::NewTab)),
        send(keys, t.tr("menu.file.open"), None, None, Msg::Repo(RepoMsg::PickRepo)),
        send(keys, t.tr("menu.file.clone"), None, None, Msg::Clone(CloneMsg::Dialog)),
        send(keys, t.tr("menu.file.init"), None, None, Msg::Repo(RepoMsg::InitRepo)),
        Entry::Submenu(Submenu {
            kind: SubmenuKind::Recent,
            label: t.tr("menu.file.recent").to_owned(),
            blocked: no_recent.map(str::to_owned),
            entries: recent(st),
        }),
        Entry::Separator,
        send(keys, t.tr("menu.file.reopen_tab"), Some(I::ReopenTab), no_closed, Msg::Tabs(TabsMsg::ReopenClosedTab)),
        send(keys, t.tr("menu.file.close_tab"), Some(I::CloseTab), None, Msg::Tabs(TabsMsg::CloseTab(usize::MAX))),
        Entry::Separator,
        app_item(keys, t.tr("menu.file.prefs"), Some(I::Prefs), Role::Prefs, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General))),
        Entry::Separator,
        app_item(keys, t.tr("menu.file.quit"), Some(I::Quit), Role::Quit, Msg::App(AppMsg::Quit)),
    ]
}

/// Open recent: the favorites, then the other recent folders and Clear recent….
fn recent(st: &AppState) -> Vec<Entry> {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let open = |p: &Path| send(keys, glowfly_core::native_path::shown(p), None, None, Msg::Repo(RepoMsg::Open(p.to_path_buf())));
    let mut out = Vec::new();
    let favs = favorites(&st.settings);
    if !favs.is_empty() {
        out.push(Entry::Heading(t.tr("repos.favorites.header").to_owned()));
        out.extend(favs.iter().map(|p| open(p)));
    }
    let rows = recent_rows(st);
    if !rows.is_empty() {
        if !favs.is_empty() {
            out.push(Entry::Separator);
        }
        out.push(Entry::Heading(t.tr("repos.recent.header").to_owned()));
        out.extend(rows.into_iter().map(|p| open(p)));
        out.push(send(keys, t.tr("repos.menu.clear_recent"), None, None, clear_recent_msg(st)));
    }
    out
}

/// Edit: Undo and Redo, finding commits, and copying the selected commit's SHA or message or the
/// checked-out branch's name.
pub(super) fn edit(st: &AppState) -> Vec<Entry> {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let idle = st.job.is_none();
    let (undo, undo_why) = st.undo.undo_entry(t, idle);
    let (redo, redo_why) = st.undo.redo_entry(t, idle);
    let no_repo = st.snap.is_none().then_some(t.tr("menu.common.no_repo"));
    let mut out = vec![
        send(keys, undo, Some(I::Undo), undo_why.as_deref(), Msg::Job(JobMsg::Undo)),
        send(keys, redo, Some(I::Redo), redo_why.as_deref(), Msg::Job(JobMsg::Redo)),
        Entry::Separator,
        send(keys, t.tr("menu.edit.find"), Some(I::Search), no_repo, Msg::Search(SearchMsg::Open)),
        Entry::Item(item(keys, t.tr("menu.edit.filter"), Some(I::SearchFilter), no_repo, MenuAction::ShowFilter)),
        Entry::Separator,
    ];
    out.push(match copy_sha(st) {
        Ok(copy) => {
            let label = copy.label(t);
            let msg = match (copy, &st.selection) {
                (ShaCopy::One(sha), _) => Msg::Copy(sha),
                // Joined as the graph's commit menu joins them.
                (ShaCopy::Many(_), Selection::Commits(m)) => Msg::Copy(shas_text(m)),
                (ShaCopy::Many(_), _) => Msg::Copy(String::new()),
            };
            send(keys, label, None, None, msg)
        }
        Err(why) => send(keys, t.tr("menu.edit.copy_sha"), None, Some(why), Msg::Copy(String::new())),
    });
    for (label, text) in [(t.tr("menu.edit.copy_message"), copy_message(st)), (t.tr("menu.edit.copy_branch"), copy_branch(st))] {
        let (text, why) = match text {
            Ok(text) => (text, None),
            Err(why) => (String::new(), Some(why)),
        };
        out.push(send(keys, label, None, why, Msg::Copy(text)));
    }
    out
}

/// View: zoom, the panels, full screen, the terminal and the activity log, the diff settings
/// (under "Diff layout" and "Diff shows"), the theme and Refresh.
pub(super) fn view(st: &AppState) -> Vec<Entry> {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let s = &st.settings;
    let [zoom_in, zoom_out, reset] = zoom_blocked(t, s.zoom_level());
    let mut out = vec![
        send(keys, t.tr("menu.view.zoom_in"), Some(I::ZoomIn), zoom_in.as_deref(), Msg::Layout(LayoutMsg::ZoomIn)),
        send(keys, t.tr("menu.view.zoom_out"), Some(I::ZoomOut), zoom_out.as_deref(), Msg::Layout(LayoutMsg::ZoomOut)),
        send(keys, t.tr("menu.view.zoom_reset"), Some(I::ZoomReset), reset.as_deref(), Msg::Layout(LayoutMsg::ZoomReset)),
        Entry::Separator,
    ];
    for panel in Panel::ALL {
        // A drawn title bar replaces the menu bar.
        if panel == Panel::MenuBar && st.view.chrome.drawn() {
            continue;
        }
        let on = shown(s, panel);
        let set = Msg::Layout(LayoutMsg::SetPanel { panel, shown: !on });
        out.push(marked(keys, panel_label(t, panel), Mark::Check(on), panel.binding(), None, MenuAction::send(set)));
    }
    let no_repo = st.snap.is_none().then_some(t.tr("menu.common.no_repo"));
    let full = MenuAction::send(Msg::Layout(LayoutMsg::ToggleFullscreen));
    out.push(marked(keys, t.tr("menu.view.full_screen"), Mark::Check(st.view.window.fullscreen), Some(I::ToggleFullscreen), None, full));
    let term = MenuAction::send(Msg::Terminal(TerminalMsg::Toggle));
    out.push(marked(keys, t.tr("menu.view.terminal"), Mark::Check(st.terminal.open), Some(I::ToggleTerminal), no_repo, term));
    out.push(marked(keys, t.tr("menu.view.activity_log"), Mark::Check(st.view.show_log), None, None, MenuAction::ToggleActivityLog));
    let journal = MenuAction::send(Msg::Journal(JournalMsg::Toggle));
    out.push(marked(keys, t.tr("menu.view.journal"), Mark::Check(st.journal.is_some()), None, no_repo, journal));
    out.push(Entry::Separator);
    // The diff viewer's saved settings: they apply whether or not a diff is open.
    let diff = |m: DiffMsg| MenuAction::send(Msg::Diff(m));
    out.push(Entry::Heading(t.tr("menu.view.diff_layout").to_owned()));
    for (layout, label) in [(DiffLayout::Inline, t.tr("menu.view.inline")), (DiffLayout::Split, t.tr("menu.view.split"))] {
        out.push(marked(keys, label, Mark::Radio(s.diff_layout == layout), None, None, diff(DiffMsg::SetDiffLayout(layout))));
    }
    out.push(Entry::Heading(t.tr("menu.view.diff_show").to_owned()));
    for (mode, label) in [(DiffMode::Hunks, t.tr("menu.view.hunks")), (DiffMode::WholeFile, t.tr("menu.view.whole_file"))] {
        out.push(marked(keys, label, Mark::Radio(s.diff_mode == mode), None, None, diff(DiffMsg::SetDiffMode(mode))));
    }
    let ignore = s.diff_ignore_whitespace;
    let ignore_msg = diff(DiffMsg::SetIgnoreWhitespace(!ignore));
    out.push(marked(keys, t.tr("menu.view.ignore_whitespace"), Mark::Toggle(ignore), None, None, ignore_msg));
    let wrap = s.diff_word_wrap;
    out.push(marked(keys, t.tr("menu.view.wrap"), Mark::Toggle(wrap), None, None, diff(DiffMsg::SetWordWrap(!wrap))));
    out.push(Entry::Submenu(Submenu {
        kind: SubmenuKind::Theme,
        label: t.tr("menu.view.theme").to_owned(),
        blocked: None,
        entries: themes(st),
    }));
    out.push(Entry::Separator);
    out.push(send(keys, t.tr("menu.view.refresh"), Some(I::Refresh), None, Msg::Repo(RepoMsg::Refresh)));
    out
}

/// The check item's label for `panel`.
fn panel_label(t: Lang, panel: Panel) -> &'static str {
    match panel {
        Panel::Left => t.tr("menu.view.left_panel"),
        Panel::Details => t.tr("menu.view.details_panel"),
        Panel::Toolbar => t.tr("menu.view.toolbar"),
        Panel::MenuBar => t.tr("menu.view.menu_bar"),
    }
}

/// The built-in themes, then the theme files, then the Preferences page.
fn themes(st: &AppState) -> Vec<Entry> {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let current = &st.settings.theme;
    let pick = |choice: ThemeChoice, label: String| {
        let on = *current == choice;
        marked(keys, label, Mark::Radio(on), None, None, MenuAction::send(Msg::Prefs(PrefsMsg::SetTheme(choice))))
    };
    let mut out: Vec<Entry> = ThemeChoice::BUILT_IN.into_iter().map(|c| pick(c.clone(), theme_names::choice(t, &c))).collect();
    let files = &st.themes.catalog.files;
    if !files.is_empty() {
        out.push(Entry::Separator);
    }
    out.extend(files.iter().map(|f| pick(ThemeChoice::Custom(f.id.clone()), theme_names::file_entry(t, f))));
    out.push(Entry::Separator);
    out.push(send(keys, t.tr("menu.view.theme_settings"), None, None, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ui))));
    out
}

/// Window: the open tabs (with their Go to tab keys), stepping between them and closing others.
pub(super) fn window(st: &AppState) -> Vec<Entry> {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let n = st.tabs.len();
    let mut out: Vec<Entry> = (0..n)
        .map(|i| {
            let tab = item(keys, tab_name(st, i), tab_binding(i, n), None, MenuAction::send(Msg::Tabs(TabsMsg::SelectTab(i))));
            Entry::Item(Item { chord: tab_chord(keys, i, n), mark: Mark::Selected(i == st.active_tab), ..tab })
        })
        .collect();
    out.push(Entry::Separator);
    let one = (n < 2).then_some(t.tr("menu.window.one_tab"));
    out.push(send(keys, t.tr("menu.window.next_tab"), Some(I::NextTab), one, Msg::Tabs(TabsMsg::CycleTab(1))));
    out.push(send(keys, t.tr("menu.window.prev_tab"), Some(I::PrevTab), one, Msg::Tabs(TabsMsg::CycleTab(-1))));
    let active = st.active_tab;
    let no_others = one.map(|_| t.tr("menu.window.no_others"));
    out.push(send(keys, t.tr("menu.window.close_others"), None, no_others, Msg::Tabs(TabsMsg::CloseOtherTabs(active))));
    let none_right = (active + 1 >= n).then_some(t.tr("menu.window.none_right"));
    out.push(send(keys, t.tr("menu.window.close_right"), None, none_right, Msg::Tabs(TabsMsg::CloseTabsToRight(active))));
    out
}

/// Help: the shortcut sheet, the palette, the welcome and the tour, the logs, the project's links,
/// the release notes and Check for updates, and About.
pub(super) fn help(st: &AppState) -> Vec<Entry> {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let mut out = vec![
        send(keys, t.tr("menu.help.shortcuts"), Some(I::Sheet), None, Msg::Shortcut(KeyAction::ToggleSheet)),
        send(keys, t.tr("menu.help.palette"), Some(I::Palette), None, Msg::Palette(PaletteMsg::TogglePalette)),
        Entry::Separator,
        send(keys, t.tr("menu.help.welcome"), None, None, Msg::Onboarding(OnboardingMsg::ShowWelcome)),
        send(keys, t.tr("menu.help.tour"), None, None, Msg::Onboarding(OnboardingMsg::StartTour)),
        Entry::Separator,
    ];
    let logs = [
        (t.tr("menu.help.activity_log"), LogKind::Activity),
        (t.tr("menu.help.error_log"), LogKind::Errors),
        (t.tr("menu.help.performance_log"), LogKind::Performance),
    ];
    out.extend(logs.into_iter().map(|(label, kind)| send(keys, label, None, None, Msg::Logs(LogsMsg::Show(kind)))));
    let checking = st.updates.busy().then_some(t.tr("menu.help.busy"));
    out.extend([
        send(keys, t.tr("menu.help.logs_folder"), None, None, Msg::Logs(LogsMsg::OpenFolder)),
        send(keys, t.tr("menu.help.export_logs"), None, None, Msg::Logs(LogsMsg::Export)),
        Entry::Separator,
        send(keys, t.tr("menu.help.website"), None, None, Msg::OpenUrl(WEBSITE.to_owned())),
        send(keys, t.tr("menu.help.report_issue"), None, None, Msg::OpenUrl(ISSUES.to_owned())),
        Entry::Separator,
        send(keys, t.tr("menu.help.release_notes"), None, None, Msg::Updates(UpdateMsg::ShowNotes)),
        send(keys, t.tr("menu.help.check_updates"), None, checking, Msg::Updates(UpdateMsg::CheckNow)),
        app_item(keys, t.tr("menu.help.about"), None, Role::About, Msg::App(AppMsg::ShowAbout)),
    ]);
    out
}
