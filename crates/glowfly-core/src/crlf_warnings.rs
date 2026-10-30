//! Git's line-ending warnings in a job's output, folded into one line for the job log: on
//! Windows (`core.autocrlf`) or with `text` attributes, `git add`, `stash`, `checkout` and more
//! print one per file, which buries what the job said. Only git's exact warning shapes are
//! folded; every other line stays. Pure.

use std::borrow::Cow;

/// What git 2.37 and later print before the path.
const PREFIX: &str = "warning: in the working copy of '";

/// What may follow the path's closing quote: the conversion, with or without git's tail.
const ENDINGS: [&str; 4] = [
    ", LF will be replaced by CRLF the next time Git touches it",
    ", CRLF will be replaced by LF the next time Git touches it",
    ", LF will be replaced by CRLF",
    ", CRLF will be replaced by LF",
];

/// Whether `line` is one of git's "LF will be replaced by CRLF" or "CRLF will be replaced by
/// LF" warnings.
pub fn is_warning(line: &str) -> bool {
    let Some(rest) = line.trim_end_matches('\r').strip_prefix(PREFIX) else { return false };
    rest.rsplit_once('\'').is_some_and(|(path, end)| !path.is_empty() && ENDINGS.contains(&end))
}

/// `output` with its line-ending warnings, when there are two or more, replaced by `line(n)` (n:
/// how many were folded) where the first one was. One warning alone is kept as git wrote it.
pub fn fold(output: &str, line: impl FnOnce(usize) -> String) -> Cow<'_, str> {
    let n = output.lines().filter(|l| is_warning(l)).count();
    if n < 2 {
        return Cow::Borrowed(output);
    }
    let mut folded = Some(line(n));
    let mut out: Vec<Cow<str>> = Vec::new();
    for l in output.lines() {
        if !is_warning(l) {
            out.push(Cow::Borrowed(l));
        } else if let Some(f) = folded.take() {
            out.push(Cow::Owned(f));
        }
    }
    Cow::Owned(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{fold, is_warning};

    fn said(n: usize) -> String {
        format!("({n} line-ending warnings)")
    }

    #[test]
    fn folds_two_or_more_where_the_first_was() {
        let out = "$ git add -A\n\
            warning: in the working copy of 'a.txt', LF will be replaced by CRLF the next time Git touches it\n\
            warning: in the working copy of 'd/it''s.txt', CRLF will be replaced by LF the next time Git touches it\n\
            warning: in the working copy of 'b', LF will be replaced by CRLF\n\
            $ git commit -m x\n\
            [main 1a2b3c4] x";
        assert_eq!(fold(out, said), "$ git add -A\n(3 line-ending warnings)\n$ git commit -m x\n[main 1a2b3c4] x");
    }

    #[test]
    fn one_warning_and_other_lines_stay() {
        let one = "warning: in the working copy of 'a', LF will be replaced by CRLF\nerror: nope";
        assert_eq!(fold(one, said), one);
        for kept in [
            "warning: in the working copy of 'a', LF will be replaced by CRLF soon",
            "warning: LF will be replaced by CRLF in a.txt.",
            "warning: in the working copy of '', LF will be replaced by CRLF",
            "hint: in the working copy of 'a', LF will be replaced by CRLF",
            "remote: warning: in the working copy of 'a', LF will be replaced by CRLF",
        ] {
            assert!(!is_warning(kept), "{kept}");
        }
        assert!(is_warning("warning: in the working copy of 'a', CRLF will be replaced by LF\r"));
    }

    #[test]
    fn folds_what_git_prints() {
        let dir = crate::testutil::scratch_repo("crlf-fold");
        crate::testutil::write(&dir, "a.txt", "a\n");
        crate::testutil::write(&dir, "b.txt", "b\n");
        let env = crate::testutil::empty_global_config(&dir);
        let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let a = crate::gitcli::args(&["-c", "core.autocrlf=true", "-c", "core.safecrlf=warn", "add", "."]);
        let (ok, out) = crate::gitcli::run_with_input(&dir, &a, "", &env);
        assert!(ok, "{out}");
        assert_eq!(fold(&out, said).trim(), "(2 line-ending warnings)", "{out}");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(dir.with_file_name("crlf-fold-gitconfig"));
    }
}
