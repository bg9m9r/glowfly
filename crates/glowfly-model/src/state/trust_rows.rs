//! The trust prompt's setting and hook rows as the dialog draws them: which part of a shown
//! value is the program git runs (`value_parts`, emphasised), and what a row's recognition is
//! checked against (`row_follows`), and a step's text with what a row shows of a long one
//! (`step_rows`). Display only: nothing here changes what is trusted.

use super::TrustPrompt;
use glowfly_core::trust::{Follow, TrustScan};
use std::ops::Range;

/// Characters that make a value a shell snippet rather than one program and its arguments.
const SHELL: &[char] = &['!', ';', '|', '&', '$', '(', ')', '<', '>', '`', '"', '\''];

/// The byte range of the program word in `shown` (a `shown_value`): its first
/// whitespace-separated word, when the value is a plain command line. None for a shell snippet
/// (any of `SHELL`), which has no single program, and for a blank value.
pub fn value_parts(shown: &str) -> Option<Range<usize>> {
    if shown.contains(SHELL) {
        return None;
    }
    let start = shown.find(|c: char| !c.is_whitespace())?;
    let end = shown[start..].find(char::is_whitespace).map_or(shown.len(), |n| start + n);
    Some(start..end)
}

/// The places in the working tree whose files a setting of the repository (`sub`: None for
/// the top one, or a submodule's path) may name, for `known::setting`: those of the scan
/// shown and of the whole scan both, since a gained or incoming scan lists only what is new
/// and a `git-lfs` file found earlier still makes Git LFS's settings run it.
pub fn row_follows(p: &TrustPrompt, sub: Option<&str>) -> Vec<Follow> {
    let shown = p.incoming.as_deref().or(p.gained.as_ref()).unwrap_or(&p.scan);
    let of = |s: &TrustScan| -> Vec<Follow> {
        match sub {
            None => s.follows.clone(),
            Some(path) => s.submodules.iter().filter(|m| m.path == path).flat_map(|m| m.scan.follows.clone()).collect(),
        }
    };
    let mut out = of(shown);
    out.extend(of(&p.scan).into_iter().filter(|f| !out.contains(f)).collect::<Vec<_>>());
    out
}

/// The most characters of a step its row shows before Show all.
const STEP_CHARS: usize = 400;

/// A step of a rebase or pick in progress as the dialog shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct StepRow {
    /// All of it, control characters made visible.
    pub whole: String,
    /// For a step longer than `STEP_CHARS`: its first `STEP_CHARS` characters with "…", and
    /// how many more there are.
    pub cut: Option<(String, usize)>,
}

/// The rows of `steps` (`TrustScan::steps`), in order. The decision covers each whole step, so
/// a cut row says how much is left out and the dialog offers to show all of it.
pub fn step_rows(steps: &[String]) -> Vec<StepRow> {
    let row = |step: &String| {
        let whole = glowfly_core::text::printable(step);
        let cut = whole.char_indices().nth(STEP_CHARS).map(|(at, _)| (format!("{}…", &whole[..at]), whole[at..].chars().count()));
        StepRow { whole, cut }
    };
    steps.iter().map(row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::trust::SubmoduleScan;
    use std::path::PathBuf;

    fn part(v: &str) -> Option<&str> {
        value_parts(v).map(|r| &v[r])
    }

    #[test]
    fn the_program_word_is_the_first_word_of_a_plain_command() {
        assert_eq!(part("git-lfs clean -- %f"), Some("git-lfs"));
        assert_eq!(part("ssh -i ~/.ssh/key"), Some("ssh"));
        assert_eq!(part(".git/fsmon.sh"), Some(".git/fsmon.sh"));
        assert_eq!(part("!f() { echo; }; f"), None);
        assert_eq!(part("\"C:/x y/z.exe\" --a"), None);
        assert_eq!(part("vim | tee"), None);
        assert_eq!(part("   "), None);
    }

    fn follow(path: &str) -> Follow {
        Follow { path: path.into(), tracked: None, hooks: false }
    }

    #[test]
    fn a_long_step_is_cut_with_the_count_left_out() {
        let long = format!("exec {}", "é".repeat(STEP_CHARS));
        let rows = step_rows(&["exec make".into(), long.clone(), "x".repeat(STEP_CHARS)]);
        assert_eq!(rows[0], StepRow { whole: "exec make".into(), cut: None });
        let (cut, more) = rows[1].cut.as_ref().expect("longer than a row shows");
        assert_eq!((cut.chars().count(), *more), (STEP_CHARS + 1, 5));
        assert!(cut.ends_with('…') && rows[1].whole == long);
        assert_eq!(rows[2].cut, None, "exactly a row's worth isn't cut");
    }

    #[test]
    fn rows_check_the_whole_scan_s_places_too() {
        let sub = |f: &str| SubmoduleScan { path: "lib".into(), scan: TrustScan { follows: vec![follow(f)], ..TrustScan::default() } };
        let scan = TrustScan { follows: vec![follow("git-lfs")], submodules: vec![sub("git-lfs.exe")], ..TrustScan::default() };
        let gained = TrustScan { follows: vec![follow("tools/x")], submodules: vec![sub("bin/y")], ..TrustScan::default() };
        let p = TrustPrompt {
            name: "tools".into(),
            workdir: PathBuf::from("C:/work/tools"),
            real: PathBuf::from("C:/work/tools"),
            scan,
            refused: false,
            held: None,
            gained: Some(gained),
            terminal: false,
            incoming: None,
            unchecked: None,
            open_step: None,
        };
        assert_eq!(row_follows(&p, None), [follow("tools/x"), follow("git-lfs")]);
        assert_eq!(row_follows(&p, Some("lib")), [follow("bin/y"), follow("git-lfs.exe")]);
        assert!(row_follows(&p, Some("other")).is_empty());
    }
}
