//! Rendering tests: whole-window snapshots compared against approved images in
//! `tests/snapshots`. Run `UPDATE_SNAPSHOTS=1 cargo test -p trellis` to accept changes, then
//! review the new images.

use super::{UiCache, avatars::Avatars, draw, theme::color};
use crate::state::{AppState, PrefPage, PrefsState, Selection, Tab};
use eframe::egui;
use egui_kittest::Harness;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::gitcli::{args, run_with_input};
use trellis_core::highlight;
use trellis_core::intraline;
use trellis_core::repo::{self, DiffTarget, LoadOptions};
use trellis_core::settings::{DiffLayout, Settings, ThemeChoice};
use trellis_core::spell::Speller;

const SIZE: egui::Vec2 = egui::vec2(1400.0, 820.0);

/// Fixed identity and dates keep commit SHAs and the rendering identical between runs.
fn git(dir: &Path, a: &[&str], date: &str) {
    let (ok, out) = try_git(dir, a, date);
    assert!(ok, "git {a:?} failed: {out}");
}

fn try_git(dir: &Path, a: &[&str], date: &str) -> (bool, String) {
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
    let dir = std::env::temp_dir().join(format!("trellis-snap-{}-{name}", std::process::id())).join("demo");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let d = "2026-01-05T10:00:00+00:00";
    git(&dir, &["init", "-q", "-b", "main"], d);
    git(&dir, &["config", "commit.gpgsign", "false"], d);
    let write = |f: &str, t: &str| std::fs::write(dir.join(f), t).unwrap();
    write("README.md", "# Demo\n");
    git(&dir, &["add", "."], d);
    git(&dir, &["commit", "-q", "-m", "Initial commit"], d);
    write("app.txt", "app\n");
    git(&dir, &["add", "."], "2026-01-06T10:00:00+00:00");
    git(&dir, &["commit", "-q", "-m", "Add app skeleton", "-m", "Project layout and CI."], "2026-01-06T10:00:00+00:00");
    git(&dir, &["checkout", "-q", "-b", "login-page"], d);
    write("login.txt", "login\n");
    git(&dir, &["add", "."], "2026-01-07T10:00:00+00:00");
    git(&dir, &["commit", "-q", "-m", "Add login page"], "2026-01-07T10:00:00+00:00");
    git(&dir, &["checkout", "-q", "main"], d);
    write("app.txt", "app\nfix\n");
    git(&dir, &["commit", "-q", "-am", "Fix build on Windows"], "2026-01-08T10:00:00+00:00");
    git(&dir, &["merge", "-q", "--no-ff", "login-page", "-m", "Merge branch 'login-page'"], "2026-01-09T10:00:00+00:00");
    git(&dir, &["tag", "v0.1.0"], d);
    git(&dir, &["checkout", "-q", "-b", "search"], d);
    write("search.txt", "search\n");
    git(&dir, &["add", "."], "2026-01-10T10:00:00+00:00");
    git(&dir, &["commit", "-q", "-m", "Search results page"], "2026-01-10T10:00:00+00:00");
    git(&dir, &["checkout", "-q", "main"], d);
    write("README.md", "# Demo\n\nWork in progress.\n");
    dir
}

fn settings(theme: ThemeChoice) -> Settings {
    // Fixed locale and initials (no network) for reproducible images.
    Settings { theme, date_locale: "en_US".into(), use_initials: true, auto_fetch_minutes: 0, ..Settings::default() }
}

/// 2026-02-01 12:00 UTC, shown in UTC: rendering never depends on the machine's clock or zone.
const NOW: i64 = 1_769_947_200;

fn render(name: &str, mut st: AppState, dark: bool) {
    st.view.now = NOW;
    st.view.utc_offset = 0;
    let mut cache: Option<UiCache> = None;
    let mut harness = Harness::builder().with_size(SIZE).build_state(
        move |ctx, st: &mut AppState| {
            // Like the app, pick the palette at the start of every frame.
            color::set_dark(dark);
            super::theme::apply_theme(ctx);
            ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
            let cache = cache.get_or_insert_with(|| UiCache { avatars: Avatars::default(), speller: Speller::new() });
            draw(ctx, st, cache, &mut Vec::new());
        },
        std::mem::replace(&mut st, AppState::new(Settings::default(), Vec::new(), Instant::now())),
    );
    harness.run();
    harness.snapshot(name);
}

fn with_repo(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let dir = fixture_repo(name);
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.selection = Selection::Wip;
    // A second, inactive tab.
    st.tabs = vec![Tab::unloaded(dir.clone()), Tab::unloaded(PathBuf::from("C:/work/api-server"))];
    (st, dir)
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

/// The fixture's app.txt with an uneven edit: one line replaced, one added. `name` keeps
/// tests running in parallel apart.
fn with_diff(layout: DiffLayout, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("{name}-{layout:?}"));
    std::fs::write(dir.join("app.txt"), "app\nfixed build\nnew line\n").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap()); // lists app.txt as changed
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_inline_and_split() {
    let (st, dir) = with_diff(DiffLayout::Inline, "diff");
    render("diff_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_diff(DiffLayout::Split, "diff");
    render("diff_split", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_line_selection() {
    use crate::msg::Msg;
    use crate::state::lines::LineClick;
    for (layout, name) in [(DiffLayout::Inline, "diff_lines_inline"), (DiffLayout::Split, "diff_lines_split")] {
        let (mut st, dir) = with_diff(layout, "diff-lines");
        // Select `-fix` and `+new line`, as a click and a Ctrl-click would.
        let lines = &st.diff.as_ref().unwrap().1.hunks[0].lines;
        let at = |origin: char, text: &str| lines.iter().position(|l| l.origin == origin && l.text == text).unwrap();
        let (fix, new_line) = (at('-', "fix"), at('+', "new line"));
        for (line, how) in [(fix, LineClick::Only), (new_line, LineClick::Toggle)] {
            crate::update::update(&mut st, Msg::SelectDiffLine { hunk: 0, line, how }, Instant::now());
        }
        assert_eq!(st.view.diff_lines.count_in(&DiffTarget::Unstaged("app.txt".into()), 0), 2);
        render(name, st, true);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

/// A Rust file with a doc comment, strings, numbers, keywords, a type and a function name;
/// the edit replaces one line and adds another.
fn with_rust_diff(layout: DiffLayout, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, &format!("diff-syntax-{layout:?}"));
    let before = "/// Prints a greeting.
fn main() {
    let name = \"world\";
    let count = 3;
    println!(\"hello {name} x{count}\");
}
";
    let after = before.replace("let count = 3;", "let count: u32 = 5; // more greetings").replace(
        "x{count}\");
",
        "x{count}\");
    std::process::exit(0);
",
    );
    std::fs::write(dir.join("main.rs"), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "main.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add main.rs"], d);
    std::fs::write(dir.join("main.rs"), after).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("main.rs".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    assert!(diff.highlight.is_some(), "main.rs is highlighted as Rust");
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_syntax_highlighting() {
    let (st, dir) = with_rust_diff(DiffLayout::Inline, ThemeChoice::Dark);
    render("diff_syntax_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_rust_diff(DiffLayout::Split, ThemeChoice::Light);
    render("diff_syntax_split_light", st, false);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// A one-word edit in a Rust file, highlighted and word-marked as the runner does.
fn with_word_diff(layout: DiffLayout, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, &format!("diff-words-{layout:?}"));
    let before = "fn total(price: u32, quantity: u32) -> u32 {
    let total = price * quantity;
    total
}
";
    std::fs::write(dir.join("words.rs"), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "words.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add words.rs"], d);
    std::fs::write(dir.join("words.rs"), before.replace("price * quantity;", "price * count;")).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("words.rs".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_word_highlights() {
    let (st, dir) = with_word_diff(DiffLayout::Inline, ThemeChoice::Dark);
    let d = &st.diff.as_ref().unwrap().1;
    let marked = d.hunks[0].lines.iter().enumerate().filter(|(l, _)| !d.intraline.line(0, *l).is_empty()).count();
    assert_eq!(marked, 2, "the -/+ pair is marked");
    render("diff_words_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_word_diff(DiffLayout::Split, ThemeChoice::Light);
    render("diff_words_split_light", st, false);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn preferences_ui_customization() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    render("prefs_ui", st, true);
}

#[test]
fn preferences_commit_signing() {
    use crate::state::prefs::GitConfigValues;
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
    render("prefs_signing", st, true);
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
    })));
    render("publish_dialog", st, true);
}

#[test]
fn ghost_branch_on_the_current_commit() {
    // Selecting the checked-out commit shows "+ branch" after its labels; other rows show none.
    let (mut st, _dir) = with_repo(ThemeChoice::Dark, "ghost");
    let head = st.snap.as_ref().and_then(|s| s.head_oid).expect("HEAD");
    st.selection = Selection::Commit(head);
    render("graph_ghost_branch", st, true);
}

/// The fixture in the middle of merging `theme-tweaks`, which conflicts with main in `file`
/// (written by `content` for each side: "base", "ours" or "theirs"). The view is open on it.
fn with_conflict(name: &str, file: &str, content: impl Fn(&str) -> Vec<u8>) -> (AppState, PathBuf) {
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
    view.choices[0] = trellis_core::repo::Choice::Ours;
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
