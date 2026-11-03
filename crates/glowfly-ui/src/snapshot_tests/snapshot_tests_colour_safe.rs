//! The colour-blind safe colours (Preferences > Accessibility > Colours): a diff with plain,
//! changed-word and selected lines beside a file list with added, modified, deleted and renamed
//! files, inline in Graphite, Porcelain and high contrast light and split in Graphite; and the
//! graph in Graphite with its lanes dashed by their distance from HEAD's (three side branches:
//! dashed, dotted and dash-dot).

use super::snapshot_tests::{git, harness_palette, reload, remove_fixture};
use super::snapshot_tests_branch_colour::with_branches;
use super::snapshot_tests_diff::with_diff;
use super::theme;
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use crate::state::lines::LineClick;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::color::Palette;
use glowfly_core::intraline;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::settings::{ColourPalette, DiffLayout, ThemeChoice};
use std::time::Instant;

const D: &str = "2026-01-12T10:00:00+00:00";

fn show(name: &str, st: AppState, p: Palette) {
    let mut h = harness_palette(st, theme::applied(p, ColourPalette::ColourBlind));
    h.run();
    h.check_snapshot(name);
}

/// `with_diff` with a deleted, a renamed (staged) and a new file besides app.txt and the
/// README; app.txt's edit pairs `-fix the build`/`+fixed the build` and `-log it`/`+logs it`
/// (the changed words filled) and adds `+new line`, of which `-log it` and `+new line` are
/// selected, so the diff shows plain, selected and changed-word lines on both sides.
fn diff(layout: DiffLayout, p: Palette, name: &str) {
    let (mut st, dir) = with_diff(layout, name);
    st.settings.colour_palette = ColourPalette::ColourBlind;
    std::fs::write(dir.join("app.txt"), "app\nfix the build\nlog it\n").unwrap();
    std::fs::write(dir.join("old-notes.txt"), "old\n").unwrap();
    git(&dir, &["add", "app.txt", "old-notes.txt"], D);
    git(&dir, &["commit", "-q", "-m", "Keep old notes"], D);
    std::fs::write(dir.join("app.txt"), "app\nfixed the build\nlogs it\nnew line\n").unwrap();
    std::fs::remove_file(dir.join("old-notes.txt")).unwrap();
    git(&dir, &["mv", "login.txt", "sign-in.txt"], D);
    std::fs::write(dir.join("notes.txt"), "notes\n").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut d = repo::load_diff(&dir, &target).unwrap();
    d.intraline = intraline::intraline(&d);
    st.session.diff.view = Some((target, d.into()));
    let lines = &st.session.diff.view.as_ref().unwrap().1.hunks[0].lines;
    let at = |origin: char, text: &str| lines.iter().position(|l| l.origin == origin && l.text == text).unwrap();
    let (log, new_line) = (at('-', "log it"), at('+', "new line"));
    for (line, how) in [(log, LineClick::Only), (new_line, LineClick::Toggle)] {
        crate::update::update(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how }), Instant::now());
    }
    show(name, st, p);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_inline_colour_safe_dark() {
    diff(DiffLayout::Inline, theme::GRAPHITE, "diff_inline_colour_safe_dark");
}

#[test]
fn diff_inline_colour_safe_light() {
    diff(DiffLayout::Inline, theme::PORCELAIN, "diff_inline_colour_safe_light");
}

#[test]
fn diff_inline_colour_safe_hc_light() {
    diff(DiffLayout::Inline, theme::HIGH_CONTRAST_LIGHT, "diff_inline_colour_safe_hc_light");
}

#[test]
fn diff_split_colour_safe_dark() {
    diff(DiffLayout::Split, theme::GRAPHITE, "diff_split_colour_safe_dark");
}

#[test]
fn graph_colour_safe_dark() {
    let (mut st, dir) = with_branches(ThemeChoice::Graphite, "colour-safe-graph");
    // A third side branch, so the graph shows all four lane patterns.
    git(&dir, &["checkout", "-q", "-b", "docs", "main"], D);
    std::fs::write(dir.join("docs.txt"), "docs").unwrap();
    git(&dir, &["add", "docs.txt"], D);
    git(&dir, &["commit", "-q", "-m", "Write the docs"], D);
    git(&dir, &["checkout", "-q", "main"], D);
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.settings.colour_palette = ColourPalette::ColourBlind;
    let snap = st.snap.as_mut().unwrap();
    snap.set_colouring(st.settings.colouring_for(&snap.real_workdir));
    show("graph_colour_safe_dark", st, theme::GRAPHITE);
    remove_fixture(&dir);
}
