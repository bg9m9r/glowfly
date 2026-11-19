use super::*;
use crate::diff_rows::{LineSide, row_of_file_line};
use crate::repo::{self, DiffLine, DiffTarget, Hunk, LoadDiffOptions};
use crate::settings::DiffMode;
use crate::testutil::{git, scratch_repo, write};
use crate::whole_file::{self, MIN_FOLD};
use std::sync::Arc;

/// A hunk starting after `old` old and `new` new lines, its lines numbered from there.
fn hunk(old: u32, new: u32, origins: &str) -> Hunk {
    let (mut o, mut n) = (old, new);
    let lines: Vec<DiffLine> = origins
        .chars()
        .map(|origin| {
            let on_old = matches!(origin, ' ' | '-');
            let on_new = matches!(origin, ' ' | '+');
            o += on_old as u32;
            n += on_new as u32;
            DiffLine { origin, old: on_old.then_some(o), new: on_new.then_some(n), text: format!("{origin}{o}/{n}") }
        })
        .collect();
    let olds = lines.iter().filter(|l| l.old.is_some()).count();
    let news = lines.iter().filter(|l| l.new.is_some()).count();
    let header = format!("@@ -{},{olds} +{},{news} @@", if olds == 0 { old } else { old + 1 }, new + 1);
    Hunk { header, lines }
}

fn diff(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None, old_lines: None }
}

/// Three hunks: one adding a line (old 1-5, new 1-6), one removing one (old 10-16, new 11-16),
/// and an insertion with no old lines after old line 19 (new 20-21).
fn three() -> FileDiff {
    diff(vec![hunk(0, 0, " -++   "), hunk(9, 10, "   -   "), hunk(19, 19, "++")])
}

/// Two hunks in the middle of a 30-line file: old 6-8 and 20-22 (a line replaced in each).
fn middle() -> FileDiff {
    FileDiff { old_lines: Some(30), ..diff(vec![hunk(5, 5, " -+ "), hunk(19, 19, " -+ ")]) }
}

/// `ex` with every gap of `d` open and placeholder text for each.
fn opened(d: &FileDiff) -> Expanded {
    let mut ex = Expanded::default();
    let lines = gaps(d)
        .into_iter()
        .map(|g| {
            ex.insert(g.key);
            GapLines { span: g, lines: (0..g.len).map(|i| format!("gap {i}")).collect(), spans: vec![Vec::new(); g.len] }
        })
        .collect();
    ex.set_gap_text(Arc::new(GapText::new(lines)));
    ex
}

#[test]
fn gaps_between_hunks() {
    let d = three();
    assert_eq!(
        gaps(&d),
        [
            GapSpan { hunk: 1, old_start: 6, new_start: 7, len: 4, key: 7 },
            GapSpan { hunk: 2, old_start: 17, new_start: 17, len: 3, key: 17 },
        ]
    );
    // One hunk, adjacent hunks or a binary diff: none.
    assert!(gaps(&diff(vec![hunk(0, 0, " - ")])).is_empty());
    assert!(gaps(&diff(vec![hunk(0, 0, " - "), hunk(3, 2, "+ ")])).is_empty());
    assert!(gaps(&FileDiff { binary: true, ..three() }).is_empty());
}

#[test]
fn runs_at_the_edges() {
    let d = middle();
    assert_eq!(
        gaps(&d),
        [
            GapSpan { hunk: 0, old_start: 1, new_start: 1, len: 5, key: 1 },
            GapSpan { hunk: 1, old_start: 9, new_start: 9, len: 11, key: 9 },
            GapSpan { hunk: 2, old_start: 23, new_start: 23, len: 8, key: 23 },
        ]
    );
}

#[test]
fn one_hunk_has_its_edges() {
    let one = FileDiff { old_lines: Some(20), ..diff(vec![hunk(6, 6, " -+ ")]) };
    assert_eq!(
        gaps(&one),
        [
            GapSpan { hunk: 0, old_start: 1, new_start: 1, len: 6, key: 1 },
            GapSpan { hunk: 1, old_start: 10, new_start: 10, len: 11, key: 10 },
        ]
    );
    // Without the old side's line count there is no run below.
    assert_eq!(gaps(&FileDiff { old_lines: None, ..one.clone() }).len(), 1);
    // A hunk from line 1 to the last line has none.
    let all = FileDiff { old_lines: Some(3), ..diff(vec![hunk(0, 0, " -+ ")]) };
    assert!(gaps(&all).is_empty());
    assert!(gaps(&FileDiff { old_lines: Some(4), ..all }).len() == 1);
}

#[test]
fn no_run_below_when_the_last_hunk_reaches_the_end() {
    let end = FileDiff { old_lines: Some(8), ..diff(vec![hunk(5, 5, " -+ ")]) };
    assert_eq!(gaps(&end).iter().filter(|g| g.hunk == 1).count(), 0);
    // Markers for a missing newline are not lines of the old side.
    let marked = FileDiff { old_lines: Some(8), ..diff(vec![hunk(5, 5, " -+ >=<")]) };
    assert_eq!(gaps(&marked).iter().filter(|g| g.hunk == 1).count(), 0);
    // An old line count smaller than the hunk's end (stale): none, not a wrapped number.
    assert!(gaps(&FileDiff { old_lines: Some(3), ..end }).iter().all(|g| g.hunk == 0));
    // Unequal numbers of lines before the first hunk on the two sides: no run above.
    let skewed = FileDiff { old_lines: Some(20), ..diff(vec![hunk(5, 7, " -+ ")]) };
    assert!(gaps(&skewed).iter().all(|g| g.hunk != 0));
}

#[test]
fn filled_rows_at_the_edges() {
    let d = middle();
    let n = d.hunks.len();
    let closed = Expanded::default();
    let plain_inline = inline_rows(&d);
    let rows = inline_rows_filled(&d, &closed);
    assert_eq!(rows[..plain_inline.len()], plain_inline[..]);
    let below = gaps(&d).pop().unwrap();
    assert_eq!(rows.len(), plain_inline.len() + 1);
    assert_eq!(rows.last(), Some(&DiffRow::GapBelow(below)));
    let split = split_rows_filled(&d, &closed);
    assert_eq!(split.last(), Some(&SplitRow::GapBelow(below)));
    // Open without text: still the control row.
    let mut keys_only = Expanded::default();
    keys_only.insert(23);
    assert_eq!(inline_rows_filled(&d, &keys_only), rows);

    let ex = opened(&d);
    let rows = inline_rows_filled(&d, &ex);
    assert_eq!(rows[..5], [0, 1, 2, 3, 4].map(|line| DiffRow::GapLine { hunk: 0, line }), "the run above comes first");
    assert_eq!(rows[5], DiffRow::Header(0));
    assert!(!rows.iter().any(|r| matches!(r, DiffRow::GapBelow(_))));
    assert_eq!(rows[rows.len() - 8..], [0, 1, 2, 3, 4, 5, 6, 7].map(|line| DiffRow::GapLine { hunk: n, line }));
    assert_eq!(rows[rows.len() - 9], DiffRow::Line { hunk: 1, line: 3 });
    let split = split_rows_filled(&d, &ex);
    assert_eq!(split[..5], [0, 1, 2, 3, 4].map(|line| SplitRow::GapLine { hunk: 0, line }));
    assert_eq!(split[split.len() - 8..], [0, 1, 2, 3, 4, 5, 6, 7].map(|line| SplitRow::GapLine { hunk: n, line }));

    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let plain = header_rows(&d, layout);
        assert_eq!(header_rows_filled(&d, layout, &ex), [plain[0] + 5, plain[1] + 16], "{layout:?}: the run below shifts no header");
        let (above, between) = (gaps(&d)[0], gaps(&d)[1]);
        let headers = header_rows_filled(&d, layout, &ex);
        assert_eq!(control_row(&d, layout, &ex, &above), Some(headers[0]));
        assert_eq!(control_row(&d, layout, &ex, &between), Some(headers[1]));
        let total = if layout == DiffLayout::Inline { inline_rows_filled(&d, &ex).len() } else { split_rows_filled(&d, &ex).len() };
        let closed_total = if layout == DiffLayout::Inline { rows_len(&d, &closed) } else { split_rows_filled(&d, &closed).len() };
        assert_eq!(control_row(&d, layout, &closed, &below), Some(closed_total - 1), "the closed run's row is the last");
        assert_eq!(control_row(&d, layout, &ex, &below), Some(total - 8), "its first line, once open");
    }
    // A line of the last hunk is found on its shifted row.
    let row = row_of_file_line(&d, DiffMode::Hunks, DiffLayout::Inline, &ex, LineSide::New, 21).unwrap();
    assert!(matches!(rows[row], DiffRow::Line { hunk: 1, .. }), "{:?}", rows[row]);
}

fn rows_len(d: &FileDiff, ex: &Expanded) -> usize {
    inline_rows_filled(d, ex).len()
}

#[test]
fn filled_rows_insert_gap_lines_before_headers() {
    let d = three();
    let none = Expanded::default();
    assert_eq!(inline_rows_filled(&d, &none), inline_rows(&d));
    assert_eq!(split_rows_filled(&d, &none), split_rows(&d));
    assert_eq!(header_rows_filled(&d, DiffLayout::Inline, &none), header_rows(&d, DiffLayout::Inline));

    // Open without text: still closed.
    let mut keys_only = Expanded::default();
    keys_only.insert(7);
    assert_eq!(inline_rows_filled(&d, &keys_only), inline_rows(&d));

    let ex = opened(&d);
    let rows = inline_rows_filled(&d, &ex);
    let at = rows.iter().position(|r| *r == DiffRow::Header(1)).unwrap();
    assert_eq!(rows[at - 4..at], [0, 1, 2, 3].map(|line| DiffRow::GapLine { hunk: 1, line }));
    assert_eq!(rows[at - 5], DiffRow::Line { hunk: 0, line: 6 }, "after the previous hunk's last line");
    assert_eq!(rows.len(), inline_rows(&d).len() + 7);
    let split = split_rows_filled(&d, &ex);
    let at = split.iter().position(|r| *r == SplitRow::Header(2)).unwrap();
    assert_eq!(split[at - 3..at], [0, 1, 2].map(|line| SplitRow::GapLine { hunk: 2, line }));

    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let plain = header_rows(&d, layout);
        assert_eq!(header_rows_filled(&d, layout, &ex), [plain[0], plain[1] + 4, plain[2] + 7], "{layout:?}");
    }
    // A line after an open gap is found on its shifted row.
    let row = row_of_file_line(&d, DiffMode::Hunks, DiffLayout::Inline, &ex, LineSide::New, 20).unwrap();
    assert_eq!(rows[row], DiffRow::Line { hunk: 2, line: 0 });
    let row = row_of_file_line(&d, DiffMode::Hunks, DiffLayout::Split, &ex, LineSide::New, 21).unwrap();
    assert!(matches!(split[row], SplitRow::Pair { hunk: 2, new: Some(1), .. }));
}

#[test]
fn text_for_a_moved_gap_is_ignored() {
    let d = three();
    let ex = opened(&d);
    let text = ex.gap_text().unwrap();
    let g = gaps(&d)[0];
    assert!(text.lines_for(&g).is_some());
    assert!(text.lines_for(&GapSpan { old_start: 7, ..g }).is_none());
    assert!(text.lines_for(&GapSpan { len: 5, ..g }).is_none());
    // The diff after the first hunk is staged: the old side moved, so the gap shows closed.
    let staged = diff(vec![hunk(10, 10, "   -   "), hunk(20, 19, "++")]);
    assert!(filled(&staged, &ex).is_empty());
}

#[test]
fn a_gaps_key_is_its_whole_file_folds_key() {
    let dir = scratch_repo("gaps-fold-key");
    let body =
        |changed: &[usize]| (1..=40).map(|i| if changed.contains(&i) { format!("x{i}\n") } else { format!("{i}\n") }).collect::<String>();
    write(&dir, "a.txt", &body(&[]));
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    write(&dir, "a.txt", &body(&[5, 30]));
    let target = DiffTarget::Unstaged("a.txt".into());
    let hunks = repo::load_diff_for_view(&dir, &target, LoadDiffOptions::default()).unwrap().diff;
    let whole_opts = LoadDiffOptions { whole_file: true, ..LoadDiffOptions::default() };
    let whole = repo::load_diff_for_view(&dir, &target, whole_opts).unwrap().diff;
    let w = whole.whole.as_deref().expect("the whole file");
    let g = gaps(&hunks);
    assert_eq!(g.iter().map(|g| (g.hunk, g.key)).collect::<Vec<_>>(), [(0, 1), (1, 9), (2, 34)], "above, between and below");
    assert!(g[1].len >= MIN_FOLD);
    let folds = whole_file::folds(w);
    assert_eq!(folds.len(), 2, "between and below the changes (the run above is too short to fold): {folds:?}");
    let between = folds.iter().find(|f| f.key == g[1].key).expect("the same key");
    assert_eq!(between.len(), g[1].len);
    let below = folds.iter().find(|f| f.key == g[2].key).expect("the run below has its fold's key");
    assert_eq!(below.len(), g[2].len);
    // The run above and the fold at the top of the file are both key 1, for a change at line 10 too.
    write(&dir, "a.txt", &body(&[10]));
    let later = repo::load_diff_for_view(&dir, &target, whole_opts).unwrap().diff;
    let top = gaps(&later)[0];
    assert_eq!((top.hunk, top.key), (0, 1));
    assert!(whole_file::folds(later.whole.as_deref().unwrap()).iter().any(|f| f.key == 1));
    let _ = std::fs::remove_dir_all(&dir);
}
