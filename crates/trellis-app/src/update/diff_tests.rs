//! Headless tests for the diff viewer: layout switching, next/previous change and syntax
//! highlighting.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use std::time::Instant;
use trellis_core::diff_rows::header_rows;
use trellis_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk, HunkOp};
use trellis_core::settings::DiffLayout;

/// Three hunks of 3, 4 and 2 lines; their split rows differ from their inline rows.
fn three_hunks() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let hunks = vec![
        Hunk { header: "@@ -1,2 +1,2 @@".into(), lines: vec![line(' ', "a"), line('-', "b"), line('+', "c")] },
        Hunk { header: "@@ -9,3 +9,2 @@".into(), lines: vec![line('-', "d"), line('-', "e"), line('-', "f"), line('+', "g")] },
        Hunk { header: "@@ -20 +19,2 @@".into(), lines: vec![line('+', "h"), line('+', "i")] },
    ];
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

fn with_diff() -> AppState {
    let mut st = state();
    st.diff = Some((DiffTarget::Unstaged("f.txt".into()), three_hunks()));
    st
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

#[test]
fn set_diff_layout_saves_and_keeps_hunk() {
    let mut st = with_diff();
    let d = three_hunks();
    let inline = header_rows(&d, DiffLayout::Inline);
    let split = header_rows(&d, DiffLayout::Split);
    assert_ne!(inline[1], split[1], "the layouts place hunk 1 on different rows");
    st.view.diff_top_row = inline[1];

    let fx = run(&mut st, Msg::SetDiffLayout(DiffLayout::Split));
    assert_eq!(st.settings.diff_layout, DiffLayout::Split);
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(s) if s.diff_layout == DiffLayout::Split)));
    assert_eq!(st.saved_settings, serde_json::to_string(&st.settings).unwrap());
    assert_eq!(st.view.diff_scroll_to_row, Some(split[1]));
    assert_eq!(st.view.diff_top_row, split[1]);

    st.view.diff_scroll_to_row = None;
    assert!(run(&mut st, Msg::SetDiffLayout(DiffLayout::Split)).is_empty(), "already split");
    assert_eq!(st.view.diff_scroll_to_row, None);

    // Back to inline from inside hunk 2's rows lands on hunk 2's inline header.
    st.view.diff_top_row = split[2] + 1;
    run(&mut st, Msg::SetDiffLayout(DiffLayout::Inline));
    assert_eq!(st.view.diff_scroll_to_row, Some(inline[2]));
}

#[test]
fn set_diff_layout_without_diff_still_saves() {
    let mut st = state();
    let fx = run(&mut st, Msg::SetDiffLayout(DiffLayout::Split));
    assert_eq!(st.settings.diff_layout, DiffLayout::Split);
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    assert_eq!(st.view.diff_scroll_to_row, None);
}

#[test]
fn jump_to_change_steps_between_hunks() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let mut st = with_diff();
        st.settings.diff_layout = layout;
        let headers = header_rows(&three_hunks(), layout);
        let step = |st: &mut AppState, n: isize| {
            assert!(run(st, Msg::JumpToChange(n)).is_empty());
            st.view.diff_scroll_to_row.take()
        };
        assert_eq!(step(&mut st, 1), Some(headers[1]), "{layout:?}");
        assert_eq!(step(&mut st, 1), Some(headers[2]), "{layout:?}");
        assert_eq!(step(&mut st, 1), None, "{layout:?}: no wrap past the last hunk");
        assert_eq!(step(&mut st, -1), Some(headers[1]), "{layout:?}");
        assert_eq!(step(&mut st, -1), Some(headers[0]), "{layout:?}");
        assert_eq!(step(&mut st, -1), None, "{layout:?}: no wrap before the first hunk");
    }
    // Split rows differ from inline ones, so the targets must come from split_rows.
    assert_ne!(header_rows(&three_hunks(), DiffLayout::Inline), header_rows(&three_hunks(), DiffLayout::Split));
}

#[test]
fn jump_to_change_without_diff_is_noop() {
    let mut st = state();
    assert!(run(&mut st, Msg::JumpToChange(1)).is_empty());
    assert!(run(&mut st, Msg::JumpToChange(-1)).is_empty());
    assert_eq!(st.view.diff_scroll_to_row, None);
}

#[test]
fn opening_a_diff_scrolls_to_top_but_reload_keeps_position() {
    let mut st = with_diff();
    st.view.diff_top_row = 7;
    let target = DiffTarget::Unstaged("g.txt".into());
    let generation = st.diff_gen;
    run(&mut st, Msg::DiffLoaded { target: target.clone(), reload: false, generation, result: Ok(three_hunks()) });
    assert_eq!((st.view.diff_scroll_to_row, st.view.diff_top_row), (Some(0), 0));

    st.view.diff_scroll_to_row = None;
    st.view.diff_top_row = 7;
    run(&mut st, Msg::DiffLoaded { target, reload: true, generation, result: Ok(three_hunks()) });
    assert_eq!((st.view.diff_scroll_to_row, st.view.diff_top_row), (None, 7));
}

#[test]
fn a_parked_diff_keeps_its_hunk_when_the_layout_changes_elsewhere() {
    let mut st = with_diff();
    let d = three_hunks();
    let (inline, split) = (header_rows(&d, DiffLayout::Inline), header_rows(&d, DiffLayout::Split));
    st.view.diff_top_row = inline[1];

    run(&mut st, Msg::NewTab);
    run(&mut st, Msg::SetDiffLayout(DiffLayout::Split));
    run(&mut st, Msg::SelectTab(0));
    assert_eq!(st.view.diff_top_row, split[1], "the row is converted to the layout now in use");
    assert_eq!(st.view.diff_scroll_to_row, Some(split[1]));
}

/// Lines per hunk, to compare with a highlight's shape.
fn shape(d: &FileDiff) -> Vec<usize> {
    d.hunks.iter().map(|h| h.lines.len()).collect()
}

#[test]
fn diffs_arrive_highlighted_including_reloads() {
    // Two hunks far apart, so staging the first leaves the second to reload.
    let body = |x: &str, y: &str| {
        let filler: String = (0..12).map(|i| format!("    let f{i} = {i};\n")).collect();
        format!("fn main() {{\n    let x = {x};\n{filler}    let y = {y};\n}}\n")
    };
    let dir = scratch_repo("diff-syntax");
    std::fs::write(dir.join("main.rs"), body("1", "1")).unwrap();
    git(&dir, &["add", "main.rs"]);
    git(&dir, &["commit", "-q", "-m", "rust"]);
    std::fs::write(dir.join("main.rs"), body("2; // two", "\"why\"")).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));

    drive(&mut st, Msg::OpenDiff(DiffTarget::Unstaged("main.rs".into())));
    let (_, d) = st.diff.as_ref().expect("diff open");
    assert_eq!(d.hunks.len(), 2);
    let hl = d.highlight.as_ref().expect("highlighted on load");
    assert_eq!(hl.syntax, "Rust");
    assert_eq!(hl.hunk_lens(), shape(d));
    // `let x = 1;` became `let x = 2; // two`: the pair arrives with word marks.
    let marked = |d: &FileDiff, h: usize, origin: char| {
        d.hunks[h].lines.iter().enumerate().any(|(l, dl)| dl.origin == origin && !d.intraline.line(h, l).is_empty())
    };
    assert!(marked(d, 0, '-') && marked(d, 0, '+'), "word marks on load");
    assert_eq!(d.intraline.hunk_lens(), shape(d));

    // Staging the first hunk reloads the diff with the second one left. The reload hands the
    // diff on screen to the runner so the unchanged hunk keeps its colors, and arrives colored.
    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    let held = drive_holding_diffs(&mut st, Msg::Hunk { op: HunkOp::Stage, path: "main.rs".into(), index: 0, generation });
    assert!(held.iter().any(|e| matches!(e, Effect::LoadDiff { reload: true, previous: Some(p), .. } if p.hunks.len() == 2)), "{held:?}");
    for e in held {
        if let Some(msg) = crate::runner::execute(e) {
            drive(&mut st, msg);
        }
    }
    let (_, d) = st.diff.as_ref().expect("the second hunk is still unstaged");
    assert_eq!(d.hunks.len(), 1);
    assert_eq!(d.highlight.as_ref().map(|h| h.hunk_lens()), Some(shape(d)), "the reload comes highlighted");
    assert!(marked(d, 0, '+'), "the reload comes with word marks for `let y = \"why\"`");
    assert_eq!(d.intraline.hunk_lens(), shape(d));

    drive(&mut st, Msg::OpenDiff(DiffTarget::Staged("main.rs".into())));
    let (target, d) = st.diff.as_ref().expect("staged diff open");
    assert_eq!(*target, DiffTarget::Staged("main.rs".into()));
    assert_eq!(d.highlight.as_ref().map(|h| h.hunk_lens()), Some(shape(d)));

    // Plain text stays plain.
    drive(&mut st, Msg::OpenDiff(DiffTarget::Unstaged("a.txt".into())));
    let (_, d) = st.diff.as_ref().expect("a.txt diff open");
    assert!(!d.hunks.is_empty() && d.highlight.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turning_highlighting_off_saves_and_keeps_the_spans() {
    let mut st = with_diff();
    let mut d = three_hunks();
    d.highlight = trellis_core::highlight::highlight_diff("f.rs", &d, None);
    assert!(d.highlight.is_some());
    let want = d.highlight.clone();
    st.diff = Some((DiffTarget::Unstaged("f.rs".into()), d));

    st.settings.syntax_highlighting = false;
    let fx = run(&mut st, Msg::SettingsEdited);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if !s.syntax_highlighting));
    assert_eq!(st.diff.as_ref().unwrap().1.highlight, want, "only painting is gated, so turning it back on needs no reload");
}

/// Like `drive`, but diff loads are held back and returned, to act in the gap before they land.
pub(super) fn drive_holding_diffs(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    let mut held = Vec::new();
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for effect in update(st, m, Instant::now()) {
            match effect {
                Effect::LoadDiff { .. } => held.push(effect),
                Effect::LoadStatus { .. } | Effect::LocalOp { .. } | Effect::LoadRepo { .. } => {
                    queue.extend(crate::runner::execute(effect))
                }
                _ => {}
            }
        }
    }
    held
}

/// Lines 1..=30, with lines 2, 15 and 28 changed to `x2`, `x15`, `x28` when `changed` holds.
fn numbered(changed: &[usize]) -> String {
    (1..=30).map(|i| if changed.contains(&i) { format!("x{i}\n") } else { format!("{i}\n") }).collect()
}

#[test]
fn hunk_ops_wait_for_the_reload_after_a_hunk_op() {
    let dir = scratch_repo("diff-hunk-race");
    git(&dir, &["config", "core.autocrlf", "false"]); // compare file contents byte for byte
    std::fs::write(dir.join("big.txt"), numbered(&[])).unwrap();
    git(&dir, &["add", "big.txt"]);
    git(&dir, &["commit", "-q", "-m", "big"]);
    std::fs::write(dir.join("big.txt"), numbered(&[2, 15, 28])).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    drive(&mut st, Msg::OpenDiff(DiffTarget::Unstaged("big.txt".into())));
    assert_eq!(st.diff.as_ref().map(|(_, d)| d.hunks.len()), Some(3));
    let shown = st.hunk_ops_gen().expect("hunk buttons enabled");

    // Stage H0. Its reload is held back, so H0, H1 and H2 stay on screen.
    let held = drive_holding_diffs(&mut st, Msg::Hunk { op: HunkOp::Stage, path: "big.txt".into(), index: 0, generation: shown });
    assert_eq!(held.len(), 1, "the unstaged diff reloads");
    assert_eq!(st.diff.as_ref().map(|(_, d)| d.hunks.len()), Some(3));
    assert_eq!(st.hunk_ops_gen(), None, "the buttons are disabled until the reload lands");

    // Discard H1 as seen on screen. In the real diff index 1 is now H2, so it must be refused.
    let discard_h1 = || Msg::Hunk { op: HunkOp::Discard, path: "big.txt".into(), index: 1, generation: shown };
    assert!(run(&mut st, discard_h1()).is_empty());
    assert!(st.log.iter().any(|l| !l.ok && l.text.contains("diff changed")));
    // The same click confirmed after the reload lands is refused too.
    run(&mut st, Msg::confirm("Discard hunk", "Discard?".into(), "Discard", discard_h1()));
    for effect in held {
        let reloaded = crate::runner::execute(effect).expect("diff result");
        run(&mut st, reloaded);
    }
    assert_eq!(st.diff.as_ref().map(|(_, d)| d.hunks.len()), Some(2));
    assert!(run(&mut st, Msg::ConfirmAccepted).is_empty());
    assert_eq!(std::fs::read_to_string(dir.join("big.txt")).unwrap(), numbered(&[2, 15, 28]), "nothing discarded");

    // On the reloaded diff, index 0 is H1: discarding it keeps H2.
    let fresh = st.hunk_ops_gen().expect("enabled again");
    drive(&mut st, Msg::Hunk { op: HunkOp::Discard, path: "big.txt".into(), index: 0, generation: fresh });
    assert_eq!(std::fs::read_to_string(dir.join("big.txt")).unwrap(), numbered(&[2, 28]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn late_diff_results_do_not_replace_a_newer_diff_or_reopen_a_closed_one() {
    let dir = scratch_repo("diff-late");
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    let a = DiffTarget::Unstaged("a.txt".into());
    let b = DiffTarget::Staged("b.txt".into());
    let load = |fx: Vec<Effect>| -> Msg {
        let Ok([effect]) = <[Effect; 1]>::try_from(fx) else { panic!("expected one diff load") };
        crate::runner::execute(effect).expect("diff result")
    };

    // A is opened, then B; A's result arrives last and is ignored.
    let late_a = load(run(&mut st, Msg::OpenDiff(a)));
    let b_loaded = load(run(&mut st, Msg::OpenDiff(b.clone())));
    run(&mut st, b_loaded);
    run(&mut st, late_a);
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&b));
    assert!(st.hunk_ops_gen().is_some());

    // A reload still in flight when the diff is closed doesn't bring it back.
    let late_reload = load(run(&mut st, Msg::OpenDiff(b)));
    run(&mut st, Msg::CloseDiff);
    run(&mut st, late_reload);
    assert!(st.diff.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
