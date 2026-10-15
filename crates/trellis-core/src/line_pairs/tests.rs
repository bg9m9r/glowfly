use super::*;
use crate::repo::Hunk;

pub(crate) fn hunk(lines: &[&str]) -> Hunk {
    let lines = lines
        .iter()
        .map(|l| {
            let mut chars = l.chars();
            let origin = chars.next().expect("an origin");
            DiffLine { origin, old: None, new: None, text: chars.as_str().into() }
        })
        .collect();
    Hunk { header: "@@ -1 +1 @@".into(), lines }
}

pub(crate) fn diff(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

/// A comment inserted above two edited settings.
pub(crate) const CONFIG: [&str; 7] = [
    " [server]",
    "-host = \"localhost\"",
    "-port = 8080",
    "+# listen on all interfaces",
    "+host = \"0.0.0.0\"",
    "+port = 9090",
    " timeout = 30",
];

fn anchors(lines: &[&str]) -> Vec<(u32, u32)> {
    pair_lines(&diff(vec![hunk(lines)])).hunk(0).to_vec()
}

#[test]
fn inserted_line_above_similar_ones() {
    assert_eq!(anchors(&CONFIG), [(1, 4), (2, 5)]);
}

#[test]
fn deleted_line_above_similar_ones() {
    assert_eq!(anchors(&["-// old note", "-let a = 1;", "+let a = 2;"]), [(1, 2)]);
}

#[test]
fn dissimilar_run_has_no_anchors() {
    assert!(anchors(&["-fix", "+fixed build", "+new line"]).is_empty());
}

#[test]
fn one_by_one_run_is_not_scored() {
    assert!(anchors(&["-let a = 1;", "+let a = 2;"]).is_empty());
}

#[test]
fn order_preserving_max_similarity() {
    // A~A' and B~B', in crossed order: only one of them can share a row.
    let crossed =
        anchors(&["-let total = price * quantity;", "-println!(\"done\");", "+println!(\"finished\");", "+let total = price * count;"]);
    assert_eq!(crossed, [(0, 3)], "the more similar pair wins");
    // Equally similar lines pair in position order.
    assert_eq!(anchors(&["-x = 1", "+x = 2", "+x = 3"]), [(0, 1)]);
    assert_eq!(anchors(&["-x = 1", "-x = 2", "+x = 3"]), [(0, 2)]);
}

#[test]
fn markers_and_blank_lines_never_anchor() {
    const EOFNL: &str = "\\ No newline at end of file";
    let marker = |o: char| format!("{o}{EOFNL}");
    let (gt, lt) = (marker('>'), marker('<'));
    let lines = ["-let v = 1;", &gt, "+let v = 2;", &lt, "+let w = 3;"];
    let got = anchors(&lines);
    assert_eq!(got, [(0, 2)]);
    let d = diff(vec![hunk(&lines)]);
    for (o, n) in got {
        assert!(matches!(d.hunks[0].lines[o as usize].origin, '-') && matches!(d.hunks[0].lines[n as usize].origin, '+'));
    }
    // Two markers alone, or markers beside text, never pair.
    assert!(anchors(&[&gt, "-a", &lt, "+b"]).is_empty());
    // Blank lines are left to gap filling, even when identical.
    assert!(anchors(&["-", "-   ", "+", "+   "]).is_empty());
    assert_eq!(anchors(&["-", "-x = 1;", "+", "+x = 2;"]), [(1, 3)]);
    // Identical non-blank lines: the matching with the most total similarity wins.
    assert_eq!(anchors(&["-}", "-x = 1;", "+x = 1; // c", "+}"]), [(0, 3)]);
}

#[test]
fn runs_and_hunks_indexed_separately() {
    let d = diff(vec![
        hunk(&[" a", "-note", "-let a = 1;", "+let a = 2;", " b", "-zzz", "-let b = 1;", "+let b = 2;"]),
        hunk(&["+let c = 1;", "-other", "-let c = 2;", "+let c = 3;"]),
    ]);
    let p = pair_lines(&d);
    assert_eq!(p.hunk(0), [(2, 3), (6, 7)]);
    assert_eq!(p.hunk(1), [(2, 3)], "the '+' before the '-' is its own run");
    assert_eq!((p.len(), p.is_empty()), (3, false));
    assert!(p.hunk(5).is_empty());
    assert!(p.fits(&d));
}

#[test]
fn limits_fall_back_to_position() {
    let run = |tag: &str| [format!("-note {tag}"), format!("-let {tag} = 1;"), format!("+let {tag} = 2;")];
    let lines: Vec<String> = [run("a"), run("b"), run("c")]
        .into_iter()
        .enumerate()
        .flat_map(|(k, r)| {
            let mut v = r.to_vec();
            v.push(format!(" ctx {k}"));
            v
        })
        .collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let d = diff(vec![hunk(&refs)]);
    let all = pair_lines(&d);
    assert_eq!(all.hunk(0), [(1, 2), (5, 6), (9, 10)]);

    let limits = |run_pairs, diff_pairs, cells| Limits { run_pairs, diff_pairs, cells };
    // Each run scores 2x1 pairs.
    assert!(pair_lines_with(&d, limits(1, MAX_DIFF_PAIRS, MAX_PAIR_CELLS)).is_empty());
    assert_eq!(pair_lines_with(&d, limits(2, 4, MAX_PAIR_CELLS)).hunk(0), [(1, 2), (5, 6)], "the third run is over the diff cap");
    assert!(pair_lines_with(&d, limits(2, MAX_DIFF_PAIRS, 0)).is_empty());
    // A run that runs out of cells part-way gets no anchors, nor do later runs.
    let mut cells = MAX_PAIR_CELLS;
    let one = crate::intraline::similarity_checked("let a = 1;", "let a = 2;", &mut cells);
    assert!(one.is_ok());
    let cost = MAX_PAIR_CELLS - cells;
    assert_eq!(pair_lines_with(&d, limits(2, MAX_DIFF_PAIRS, cost)).hunk(0), [(1, 2)]);
    assert_eq!(pair_lines_with(&d, limits(2, MAX_DIFF_PAIRS, cost + 1)).hunk(0), [(1, 2)]);

    let mut binary = d.clone();
    binary.binary = true;
    assert_eq!(pair_lines(&binary), LinePairs::default());
    assert_eq!(pair_lines(&diff(Vec::new())), LinePairs::default());
}

#[test]
fn a_run_over_budget_part_way_keeps_no_anchors() {
    // Two similar pairs in one run; the budget covers the first comparison only.
    let lines = ["-note", "-let a = 1;", "-let b = 1;", "+let a = 2;", "+let b = 2;"];
    let d = diff(vec![hunk(&lines)]);
    assert_eq!(pair_lines(&d).hunk(0), [(1, 3), (2, 4)]);
    let mut cells = MAX_PAIR_CELLS;
    let _ = crate::intraline::similarity_checked("let a = 1;", "let a = 2;", &mut cells);
    let cost = MAX_PAIR_CELLS - cells;
    let limits = Limits { run_pairs: MAX_RUN_PAIRS, diff_pairs: MAX_DIFF_PAIRS, cells: cost };
    assert!(pair_lines_with(&d, limits).is_empty());
}

#[test]
fn fits_rejects_stale_pairs() {
    let d = diff(vec![hunk(&CONFIG)]);
    let good = pair_lines(&d);
    assert!(good.fits(&d));
    assert!(LinePairs::default().fits(&diff(Vec::new())));
    assert!(!LinePairs::default().fits(&d), "a default value pairs by position");
    let two = diff(vec![hunk(&CONFIG), hunk(&CONFIG)]);
    assert!(!good.fits(&two), "different hunk count");
    let with = |anchors: Vec<(u32, u32)>| LinePairs { hunks: vec![anchors] };
    assert!(!with(vec![(1, 40)]).fits(&d), "out of range");
    assert!(!with(vec![(0, 4)]).fits(&d), "an anchor on a context line");
    assert!(!with(vec![(4, 1)]).fits(&d), "added before removed");
    assert!(!with(vec![(2, 5), (1, 4)]).fits(&d), "not increasing");
    assert!(!with(vec![(1, 4), (2, 4)]).fits(&d), "not strictly increasing");
    let eofnl = diff(vec![hunk(&["-a", ">\\ No newline at end of file", "+b"])]);
    assert!(!with(vec![(1, 2)]).fits(&eofnl), "an anchor on a marker");
    assert!(with(vec![(0, 2)]).fits(&eofnl));
}
