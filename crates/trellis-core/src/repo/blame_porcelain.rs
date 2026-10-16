//! Parsing `git blame --porcelain` output, as bytes.
//!
//! Each line of the file comes as a header (`<sha> <orig line> <final line>[ <lines in group>]`),
//! the commit's details the first time that commit appears (`author`, `author-mail`, …, of
//! which only a few are read), then the line itself after one TAB. The content is taken
//! exactly as git printed it after that TAB, never trimmed.

use super::blame::{BlameCommit, BlameRun};
use crate::error::{Error, Result};
use git2::Oid;
use std::collections::HashMap;

/// What the next line of output must be.
enum Expect {
    Header,
    /// Key lines, then the content of a line blamed to commit `commit`; `fill` while its details
    /// are still to read (its first appearance).
    Details {
        commit: usize,
        fill: bool,
    },
}

fn bad() -> Error {
    Error::Decode("unexpected git blame output".into())
}

/// The commits (in order of first appearance), the lines and the runs of consecutive lines
/// blamed to one commit.
pub(super) fn parse(out: &[u8]) -> Result<(Vec<BlameCommit>, Vec<String>, Vec<BlameRun>)> {
    let mut commits: Vec<BlameCommit> = Vec::new();
    let mut index: HashMap<Oid, usize> = HashMap::new();
    let mut lines: Vec<String> = Vec::new();
    let mut runs: Vec<BlameRun> = Vec::new();
    let mut expect = Expect::Header;
    for line in out.split(|b| *b == b'\n') {
        match expect {
            Expect::Header => {
                if line.is_empty() {
                    continue; // after the last line's terminator
                }
                let (oid, final_line) = header(line).ok_or_else(bad)?;
                if final_line != lines.len() + 1 {
                    return Err(bad());
                }
                let (commit, fill) = match index.get(&oid) {
                    Some(&i) => (i, false),
                    None => {
                        index.insert(oid, commits.len());
                        let summary = if oid.is_zero() { "Not committed yet".into() } else { String::new() };
                        commits.push(BlameCommit { oid, author: String::new(), email: String::new(), time: 0, summary });
                        (commits.len() - 1, true)
                    }
                };
                expect = Expect::Details { commit, fill };
            }
            Expect::Details { commit, fill } => {
                if let Some(content) = line.strip_prefix(b"\t") {
                    let text = String::from_utf8_lossy(content);
                    let text = text.strip_suffix('\r').unwrap_or(&text);
                    lines.push(text.to_owned());
                    match runs.last_mut() {
                        Some(r) if r.commit == commit => r.len += 1,
                        _ => runs.push(BlameRun { commit, start: lines.len() - 1, len: 1 }),
                    }
                    expect = Expect::Header;
                } else if fill {
                    detail(&mut commits[commit], line);
                }
            }
        }
    }
    match expect {
        Expect::Header => Ok((commits, lines, runs)),
        Expect::Details { .. } => Err(bad()),
    }
}

/// The commit and the 1-based final line number of a header line.
fn header(line: &[u8]) -> Option<(Oid, usize)> {
    let text = std::str::from_utf8(line).ok()?;
    let mut parts = text.split(' ');
    let sha = parts.next().filter(|s| (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit()))?;
    let _orig: usize = parts.next()?.parse().ok()?;
    let final_line: usize = parts.next()?.parse().ok()?;
    if let Some(count) = parts.next() {
        count.parse::<usize>().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some((Oid::from_str(sha).ok()?, final_line))
}

/// Reads one key line into `c`; keys other than the author and summary are ignored.
fn detail(c: &mut BlameCommit, line: &[u8]) {
    let (key, value) = match line.iter().position(|b| *b == b' ') {
        Some(i) => (&line[..i], &line[i + 1..]),
        None => (line, &b""[..]),
    };
    let value = || String::from_utf8_lossy(value).into_owned();
    match key {
        b"author" => c.author = value(),
        b"author-mail" => {
            let v = value();
            c.email = v.strip_prefix('<').and_then(|v| v.strip_suffix('>')).map_or_else(|| v.clone(), str::to_owned);
        }
        b"author-time" => c.time = value().trim().parse().unwrap_or(0),
        b"summary" if !c.oid.is_zero() => c.summary = value(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";

    fn details(sha: &str, author: &str, summary: &str) -> String {
        format!(
            "author {author}\nauthor-mail <{author}@example.com>\nauthor-time 1700000000\nauthor-tz +0000\n\
             committer Someone\ncommitter-mail <s@example.com>\ncommitter-time 1700000001\ncommitter-tz +0000\n\
             summary {summary}\nprevious {sha} old.txt\nboundary\nfilename a.txt\n"
        )
    }

    #[test]
    fn metadata_is_read_on_first_appearance_only() {
        let out = format!(
            "{A} 1 1 1\n{}\tone\n{B} 2 2 1\n{}\ttwo\n{A} 3 3 1\nauthor Impostor\nsummary changed\nfilename a.txt\n\tthree\n",
            details(A, "ada", "first"),
            details(B, "grace", "second")
        );
        let (commits, lines, runs) = parse(out.as_bytes()).unwrap();
        assert_eq!(lines, ["one", "two", "three"]);
        assert_eq!(commits.len(), 2);
        assert_eq!((commits[0].author.as_str(), commits[0].email.as_str()), ("ada", "ada@example.com"));
        assert_eq!((commits[0].summary.as_str(), commits[0].time), ("first", 1_700_000_000));
        assert_eq!(commits[1].author, "grace");
        assert_eq!(
            runs,
            [BlameRun { commit: 0, start: 0, len: 1 }, BlameRun { commit: 1, start: 1, len: 1 }, BlameRun { commit: 0, start: 2, len: 1 }]
        );
    }

    #[test]
    fn content_after_the_tab_is_content() {
        let out = format!("{A} 1 1 2\n{}\tauthor  spaced \n{A} 2 2\n\t\tsummary x\n", details(A, "ada", "first"));
        let (commits, lines, runs) = parse(out.as_bytes()).unwrap();
        assert_eq!(lines, ["author  spaced ", "\tsummary x"]);
        assert_eq!(commits[0].author, "ada");
        assert_eq!(runs, [BlameRun { commit: 0, start: 0, len: 2 }], "adjacent groups of one commit are one run");
    }

    #[test]
    fn content_is_decoded_lossily_and_one_cr_stripped() {
        let mut out = format!("{A} 1 1 2\n{}\t", details(A, "ada", "first")).into_bytes();
        out.extend_from_slice(b"caf\xe9\r\r\n");
        out.extend_from_slice(format!("{A} 2 2\n\tlast").as_bytes());
        let (_, lines, _) = parse(&out).unwrap();
        assert_eq!(lines, ["caf\u{fffd}\r", "last"], "missing final newline is fine");
    }

    #[test]
    fn line_numbers_out_of_order_are_refused() {
        let out = format!("{A} 1 2 1\n{}\tone\n", details(A, "ada", "first"));
        assert!(matches!(parse(out.as_bytes()), Err(Error::Decode(_))));
        let truncated = format!("{A} 1 1 1\n{}", details(A, "ada", "first"));
        assert!(matches!(parse(truncated.as_bytes()), Err(Error::Decode(_))));
        assert!(matches!(parse(b"not a header\n"), Err(Error::Decode(_))));
    }

    #[test]
    fn groups_merge_across_headers_and_zero_oid_is_not_committed() {
        let zero = "0000000000000000000000000000000000000000";
        let out = format!("{zero} 1 1 1\n{}\tnew\n", details(zero, "Not Committed Yet", "Version of a.txt from a.txt"));
        let (commits, _, _) = parse(out.as_bytes()).unwrap();
        assert!(commits[0].oid.is_zero());
        assert_eq!(commits[0].summary, "Not committed yet");
        assert!(parse(b"").unwrap().1.is_empty());
    }
}
