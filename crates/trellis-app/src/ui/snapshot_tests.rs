//! Rendering tests: whole-window snapshots compared against approved images in
//! `tests/snapshots`. Run `UPDATE_SNAPSHOTS=1 cargo test -p trellis` to accept changes, then
//! review the new images.

use super::theme::{self, color, color::Palette};
use super::{UiCache, avatars::Avatars, draw};
use crate::msg::Msg;
use crate::state::{AppState, PrefPage, PrefsState, Selection, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::Harness;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::gitcli::{args, run_with_input};
use trellis_core::repo::{self, ChangeCounts, LoadOptions};
use trellis_core::settings::{Settings, ThemeChoice};
use trellis_core::spell::Speller;

const SIZE: egui::Vec2 = egui::vec2(1400.0, 820.0);

/// Fixed identity and dates keep commit SHAs and the rendering identical between runs.
pub(super) fn git(dir: &Path, a: &[&str], date: &str) {
    let (ok, out) = try_git(dir, a, date);
    assert!(ok, "git {a:?} failed: {out}");
}

pub(super) fn try_git(dir: &Path, a: &[&str], date: &str) -> (bool, String) {
    let env = [
        ("GIT_AUTHOR_NAME", "Ada Lovelace"),
        ("GIT_AUTHOR_EMAIL", "ada@example.com"),
        ("GIT_COMMITTER_NAME", "Ada Lovelace"),
        ("GIT_COMMITTER_EMAIL", "ada@example.com"),
        ("GIT_AUTHOR_DATE", date),
        ("GIT_COMMITTER_DATE", date),
    ];
    run_with_input(dir, &args(a), "", &env)
}

/// A small repository with a merged feature branch, an open branch, a tag and a modified file.
/// Each test passes its own `name`: tests run in parallel and must not share a folder.
fn fixture_repo(name: &str) -> PathBuf {
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-snap-{}-{name}", std::process::id())).join("demo");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    copy_dir(fixture_template(), &dir);
    // The copies' file times no longer match the index; refresh it so the repository is in the
    // state the build left it (otherwise a test that changes core.autocrlf, say, would see
    // checked-out files as modified). It exits 1 for the modified README, as intended.
    let _ = try_git(&dir, &["update-index", "-q", "--refresh"], "2026-01-05T10:00:00+00:00");
    dir
}

/// The fixture, built once per test run (its ~25 git commands were most of a view test's time)
/// and copied for each test. The fixed dates make every build identical anyway.
fn fixture_template() -> &'static Path {
    static TEMPLATE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    TEMPLATE.get_or_init(|| {
        let dir = trellis_core::paths::temp_dir().join(format!("trellis-snap-{}-template", std::process::id())).join("demo");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        build_fixture(&dir);
        dir
    })
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

fn build_fixture(dir: &Path) {
    let d = "2026-01-05T10:00:00+00:00";
    git(dir, &["init", "-q", "-b", "main"], d);
    git(dir, &["config", "commit.gpgsign", "false"], d);
    let write = |f: &str, t: &str| std::fs::write(dir.join(f), t).unwrap();
    write("README.md", "# Demo\n");
    git(dir, &["add", "."], d);
    git(dir, &["commit", "-q", "-m", "Initial commit"], d);
    write("app.txt", "app\n");
    git(dir, &["add", "."], "2026-01-06T10:00:00+00:00");
    git(dir, &["commit", "-q", "-m", "Add app skeleton", "-m", "Project layout and CI."], "2026-01-06T10:00:00+00:00");
    git(dir, &["checkout", "-q", "-b", "login-page"], d);
    write("login.txt", "login\n");
    git(dir, &["add", "."], "2026-01-07T10:00:00+00:00");
    git(dir, &["commit", "-q", "-m", "Add login page"], "2026-01-07T10:00:00+00:00");
    git(dir, &["checkout", "-q", "main"], d);
    write("app.txt", "app\nfix\n");
    git(dir, &["commit", "-q", "-am", "Fix build on Windows"], "2026-01-08T10:00:00+00:00");
    git(dir, &["merge", "-q", "--no-ff", "login-page", "-m", "Merge branch 'login-page'"], "2026-01-09T10:00:00+00:00");
    git(dir, &["tag", "v0.1.0"], d);
    git(dir, &["checkout", "-q", "-b", "search"], d);
    write("search.txt", "search\n");
    git(dir, &["add", "."], "2026-01-10T10:00:00+00:00");
    git(dir, &["commit", "-q", "-m", "Search results page"], "2026-01-10T10:00:00+00:00");
    git(dir, &["checkout", "-q", "main"], d);
    write("README.md", "# Demo\n\nWork in progress.\n");
}

pub(super) fn settings(theme: ThemeChoice) -> Settings {
    // Fixed locale and initials (no network) for reproducible images.
    // Only the menu-bar snapshots draw the bar, so other whole-window images stay as they were.
    Settings { theme, date_locale: "en_US".into(), use_initials: true, auto_fetch_minutes: 0, show_menu_bar: false, ..Settings::default() }
}

/// 2026-02-01 12:00 UTC, shown in UTC: rendering never depends on the machine's clock or zone.
pub(super) const NOW: i64 = 1_769_947_200;

/// A window-sized harness drawing `st`; the messages views push collect in its state.
pub(super) fn harness(st: AppState, dark: bool) -> Harness<'static, (AppState, Vec<Msg>)> {
    harness_sized(st, dark, SIZE)
}

/// `harness` drawing in the palette `p`.
pub(super) fn harness_palette(st: AppState, p: Palette) -> Harness<'static, (AppState, Vec<Msg>)> {
    harness_palette_sized(st, p, SIZE)
}

/// `harness` with a window of `size` (a tall one shows a long page without scrolling).
pub(super) fn harness_sized(st: AppState, dark: bool, size: egui::Vec2) -> Harness<'static, (AppState, Vec<Msg>)> {
    harness_palette_sized(st, if dark { theme::DARK } else { theme::LIGHT }, size)
}

fn harness_palette_sized(mut st: AppState, p: Palette, size: egui::Vec2) -> Harness<'static, (AppState, Vec<Msg>)> {
    st.view.now = NOW;
    st.view.utc_offset = 0;
    let mut cache: Option<UiCache> = None;
    Harness::builder().with_size(size).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            // Like the app, pick the palette at the start of every frame.
            color::set_palette(&p);
            theme::apply_style(ctx, &p);
            ctx.set_theme(theme::egui_theme(&p));
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
            });
            // Tests inject working-tree files into the lists; count them as a status read would,
            // so the WIP row's counts match the files shown.
            if let Some(snap) = st.snap.as_mut() {
                snap.changes = ChangeCounts::of(&snap.staged, &snap.unstaged);
            }
            draw(ctx, st, cache, msgs);
        },
        (st, Vec::new()),
    )
}

pub(super) fn render(name: &str, st: AppState, dark: bool) {
    let mut harness = harness(st, dark);
    harness.run();
    harness.check_snapshot(name);
}

pub(super) fn with_repo(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let dir = fixture_repo(name);
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.selection = Selection::Wip;
    // A second, inactive tab.
    st.tabs = vec![Tab::unloaded(dir.clone()), Tab::unloaded(PathBuf::from("C:/work/api-server"))];
    (st, dir)
}

/// Loads the fixture at `dir` into `st` again with `opts` (hidden or soloed refs, say).
pub(super) fn reload(st: &mut AppState, dir: &Path, opts: &LoadOptions) {
    st.snap = Some(repo::load(dir, opts).unwrap());
}

/// Removes the folder `with_repo` made for `dir`.
pub(super) fn remove_fixture(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn welcome_screen() {
    let st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    render("welcome", st, true);
}

#[test]
fn main_window_dark_and_light() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "main");
    render("main_dark", st, true);
    let (st, _) = with_repo(ThemeChoice::Light, "main");
    render("main_light", st, false);
    // Same fixture (tests in this file run in parallel, so it isn't rebuilt by another test).
    let (mut st, _) = with_repo(ThemeChoice::Dark, "main");
    let snap = st.snap.as_ref().unwrap();
    let to = snap.head_oid.unwrap();
    let from = snap.commits[snap.row_of[&to]].parents[0];
    let message = "Merge branch 'login-page'".to_owned();
    st.undo.record(trellis_core::undo::Action::Commit { branch: Some("main".into()), from, to, message, amend: false });
    render("main_undo_available", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn preferences_ui_customization() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    render("prefs_ui", st, true);
}

#[test]
fn preferences_general() {
    use crate::state::prefs::GitConfigValues;
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::General);
    prefs.git = Some(GitConfigValues { autocrlf: crate::state::prefs::Autocrlf::Input, ..Default::default() });
    st.prefs = Some(prefs);
    let program = if cfg!(windows) { "C:/Program Files/Git/cmd/git.exe" } else { "/opt/homebrew/bin/git" };
    st.git.info = Some(trellis_core::git_version::GitInfo { path: Some(program.into()), version: Some((2, 45, 1)), chosen: false });
    render("prefs_general", st, true);
}

#[test]
fn preferences_commit_signing() {
    use crate::state::prefs::GitConfigValues;
    use egui_kittest::kittest::Queryable;
    use trellis_core::signing::SigningKey;
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Signing);
    prefs.git = Some(GitConfigValues {
        user_name: "Ada Lovelace".into(),
        user_email: "ada@example.com".into(),
        signing_format: "openpgp".into(),
        signing_key: "ABCDEF0123456789".into(),
        gpgsign: true,
        ..Default::default()
    });
    prefs.signing.keys = Some(vec![SigningKey {
        id: "ABCDEF0123456789".into(),
        label: "Ada Lovelace <ada@example.com>".into(),
        expires: None,
        expired: false,
    }]);
    st.prefs = Some(prefs);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_signing");
    // The passphrase-prompt choice is edited in place.
    assert!(h.state().0.settings.gpg_prompts_in_app);
    h.get_by_label("Ask for the key's passphrase in Trellis").click();
    h.run();
    assert!(!h.state().0.settings.gpg_prompts_in_app);
}

#[test]
fn publish_to_github_dialog() {
    use crate::state::{Modal, publish::PublishForm};
    use trellis_core::forge::{Account, Provider};
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "publish");
    st.modal = Some(Modal::Publish(Box::new(PublishForm {
        workdir: dir.clone(),
        branch: "main".into(),
        accounts: vec![Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() }],
        account: 0,
        user: Some("ada".into()),
        owners: vec!["ada".into(), "acme".into()],
        owner: "ada".into(),
        name: "demo".into(),
        description: "A small demo project".into(),
        private: true,
        loading: false,
        creating: false,
        error: Some("Repository creation failed. name already exists on this account".into()),
        touched: Default::default(),
    })));
    render("publish_dialog", st, true);
}

#[test]
fn ghost_branch_on_the_current_commit() {
    // Selecting the checked-out commit shows "+ branch" after its labels; other rows show none.
    let (mut st, _dir) = with_repo(ThemeChoice::Dark, "ghost");
    let head = st.snap.as_ref().and_then(|s| s.head_oid).expect("HEAD");
    st.selection = Selection::Commit(head);
    // Room for it after main's two chips.
    st.settings.column_widths = trellis_core::settings::ColumnWidths { refs: 280.0, ..Default::default() };
    // The commit's details are still loading (a spinner): a fixed number of frames.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("graph_ghost_branch");
}

/// The fixture in the middle of merging `theme-tweaks`, which conflicts with main in `file`
/// (written by `content` for each side: "base", "ours" or "theirs"). The view is open on it.
pub(super) fn with_conflict(name: &str, file: &str, content: impl Fn(&str) -> Vec<u8>) -> (AppState, PathBuf) {
    use crate::state::conflict::ConflictView;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let d = "2026-01-11T10:00:00+00:00";
    for (side, branch) in
        [("base", None), ("theirs", Some(["checkout", "-q", "-b", "theme-tweaks"])), ("ours", Some(["checkout", "-q", "main", ""]))]
    {
        if let Some(cmd) = branch {
            let cmd: Vec<&str> = cmd.into_iter().filter(|a| !a.is_empty()).collect();
            git(&dir, &cmd, d);
        }
        std::fs::write(dir.join(file), content(side)).unwrap();
        git(&dir, &["add", file], d);
        git(&dir, &["commit", "-q", "-m", &format!("Settings: {side}")], d);
    }
    assert!(!try_git(&dir, &["merge", "theme-tweaks"], d).0, "the merge conflicts");
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let mut view = ConflictView::loading(file.into());
    view.set_file(repo::load_conflict(&dir, file).unwrap());
    st.conflict = Some(view);
    (st, dir)
}

#[test]
fn conflict_view() {
    let settings = |side: &str| {
        let mut lines: Vec<String> = (1..=12).map(|i| format!("setting_{i} = {i}")).collect();
        match side {
            "ours" => {
                lines[2] = "theme = \"dark\"".into();
                lines[8] = "font_size = 13".into();
            }
            "theirs" => {
                lines[2] = "theme = \"solarized\"\naccent = \"teal\"".into();
                lines[8] = "font_size = 14".into();
            }
            _ => {}
        }
        (lines.join("\n") + "\n").into_bytes()
    };
    let (mut st, dir) = with_conflict("conflict", "settings.txt", settings);
    let view = st.conflict.as_mut().unwrap();
    assert_eq!(view.chunk_count(), 2);
    view.picks[0] = trellis_core::repo::Choice::Ours.into();
    render("conflict_view", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_binary() {
    let (st, dir) = with_conflict("conflict-binary", "logo.bin", |side| format!("{side}\0image").into_bytes());
    assert_eq!(st.conflict.as_ref().unwrap().file.as_ref().unwrap().content, trellis_core::repo::ConflictContent::Binary);
    render("conflict_binary", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// The fixture's app.txt grown to a dozen lines, moved to src/app.txt with an edit, then
/// edited again; the file history is open on it with entry `select` shown, walked with
/// `limit` commits per request.
pub(super) fn with_history(name: &str, theme: ThemeChoice, limit: usize, select: usize) -> (AppState, PathBuf) {
    use crate::state::history::FileHistoryView;
    use trellis_core::repo::{DiffTarget, HistoryStart};
    use trellis_core::{highlight, intraline};
    let (mut st, dir) = with_repo(theme, name);
    let lines: Vec<String> = ["app", "fix"].into_iter().map(String::from).chain((3..=12).map(|i| format!("setting_{i} = {i}"))).collect();
    let text = |l: &[String]| l.join("\n") + "\n";
    std::fs::write(dir.join("app.txt"), text(&lines)).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["commit", "-q", "-m", "Expand app settings", "app.txt"], d);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    git(&dir, &["mv", "app.txt", "src/app.txt"], d);
    let mut moved = lines.clone();
    moved[5] = "setting_6 = 60".into();
    std::fs::write(dir.join("src/app.txt"), text(&moved)).unwrap();
    git(&dir, &["add", "src/app.txt"], d);
    let d = "2026-01-12T10:00:00+00:00";
    git(&dir, &["commit", "-q", "-m", "Move app settings under src"], d);
    moved[9] = "setting_10 = 100".into();
    moved.push("theme = \"dark\"".into());
    std::fs::write(dir.join("src/app.txt"), text(&moved)).unwrap();
    let d = "2026-01-14T10:00:00+00:00";
    // Only the file: the fixture's README edit stays uncommitted.
    git(&dir, &["commit", "-q", "-m", "Raise setting 10 and add a theme", "src/app.txt"], d);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());

    let h = repo::file_history(&dir, "src/app.txt", &HistoryStart::Head, limit).unwrap();
    let mut view = FileHistoryView::loading("src/app.txt".into(), None);
    view.append(h);
    view.selected = Some(select);
    let e = &view.entries[select];
    let target = DiffTarget::Commit(e.oid, e.path.clone());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    st.history = Some(view);
    (st, dir)
}

#[test]
fn file_history() {
    let (st, dir) = with_history("history", ThemeChoice::Dark, 500, 0);
    let view = st.history.as_ref().unwrap();
    let statuses: String = view.entries.iter().map(|e| e.status).collect();
    assert_eq!(statuses, "MRMMA");
    assert!(view.complete());
    render("file_history", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn file_history_partial_light() {
    let (mut st, dir) = with_history("history-partial", ThemeChoice::Light, 2, 1);
    let view = st.history.as_ref().unwrap();
    assert_eq!(view.entries.len(), 2);
    assert!(view.searched_until.is_some() && !view.resume.is_empty());
    // The renaming commit is not on the graph (as if its branch were hidden).
    let gone = view.entries[1].oid;
    st.snap.as_mut().unwrap().row_of.remove(&gone);
    render("file_history_partial_light", st, false);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
