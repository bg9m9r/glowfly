use super::*;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const P: &str = "3333333333333333333333333333333333333333";

fn details(sha: &str, author: &str, summary: &str) -> String {
    format!(
        "author {author}\nauthor-mail <{author}@example.com>\nauthor-time 1700000000\nauthor-tz +0000\n\
         committer Someone\ncommitter-mail <s@example.com>\ncommitter-time 1700000001\ncommitter-tz +0000\n\
         summary {summary}\nprevious {sha} old.txt\nboundary\nfilename a.txt\n"
    )
}

fn run(commit: usize, origin: usize, start: usize, len: usize) -> BlameRun {
    BlameRun { commit, origin, start, len }
}

#[test]
fn metadata_is_read_on_first_appearance_only() {
    let out = format!(
        "{A} 1 1 1\n{}\tone\n{B} 2 2 1\n{}\ttwo\n{A} 3 3 1\nauthor Impostor\nsummary changed\nfilename a.txt\n\tthree\n",
        details(A, "ada", "first"),
        details(B, "grace", "second")
    );
    let p = parse(out.as_bytes()).unwrap();
    assert_eq!(p.lines, ["one", "two", "three"]);
    assert_eq!(p.commits.len(), 2);
    let commits = &p.commits;
    assert_eq!((commits[0].author.as_str(), commits[0].email.as_str()), ("ada", "ada@example.com"));
    assert_eq!((commits[0].summary.as_str(), commits[0].time), ("first", 1_700_000_000));
    assert_eq!(commits[1].author, "grace");
    assert!(commits.iter().all(|c| c.body.is_empty()), "the porcelain carries no body");
    assert_eq!(p.runs, [run(0, 0, 0, 1), run(1, 1, 1, 1), run(0, 0, 2, 1)]);
    assert_eq!(p.orig_lines, [0, 1, 2]);
}

#[test]
fn content_after_the_tab_is_content() {
    let out = format!("{A} 1 1 2\n{}\tauthor  spaced \n{A} 2 2\n\t\tsummary x\n", details(A, "ada", "first"));
    let p = parse(out.as_bytes()).unwrap();
    assert_eq!(p.lines, ["author  spaced ", "\tsummary x"]);
    assert_eq!(p.commits[0].author, "ada");
    assert_eq!(p.runs, [run(0, 0, 0, 2)], "adjacent groups of one commit are one run");
}

#[test]
fn content_is_decoded_lossily_and_one_cr_stripped() {
    let mut out = format!("{A} 1 1 2\n{}\t", details(A, "ada", "first")).into_bytes();
    out.extend_from_slice(b"caf\xe9\r\r\n");
    out.extend_from_slice(format!("{A} 2 2\n\tlast").as_bytes());
    let p = parse(&out).unwrap();
    assert_eq!(p.lines, ["caf\u{fffd}\r", "last"], "missing final newline is fine");
}

#[test]
fn line_numbers_out_of_order_are_refused() {
    let out = format!("{A} 1 2 1\n{}\tone\n", details(A, "ada", "first"));
    assert!(matches!(parse(out.as_bytes()), Err(Error::Decode(_))));
    let truncated = format!("{A} 1 1 1\n{}", details(A, "ada", "first"));
    assert!(matches!(parse(truncated.as_bytes()), Err(Error::Decode(_))));
    assert!(matches!(parse(b"not a header\n"), Err(Error::Decode(_))));
    let zero_orig = format!("{A} 0 1 1\n{}\tone\n", details(A, "ada", "first"));
    assert!(matches!(parse(zero_orig.as_bytes()), Err(Error::Decode(_))));
}

#[test]
fn groups_merge_across_headers_and_zero_oid_is_not_committed() {
    let zero = "0000000000000000000000000000000000000000";
    let out = format!("{zero} 1 1 1\n{}\tnew\n", details(zero, "Not Committed Yet", "Version of a.txt from a.txt"));
    let p = parse(out.as_bytes()).unwrap();
    assert!(p.commits[0].oid.is_zero());
    assert_eq!(p.commits[0].summary, "Not committed yet");
    assert!(parse(b"").unwrap().lines.is_empty());
}

#[test]
fn previous_and_filename_make_origins() {
    // A's first lines come from a.txt (with a parent to step back to); later, git names another
    // path for A, which is a second origin and splits the run.
    let out = format!(
        "{A} 4 1 1\n{}\tone\n{A} 5 2 1\n\ttwo\n{A} 9 3 1\nprevious {P} b-old.txt\nfilename b.txt\n\tthree\n\
         {A} 10 4 1\nfilename a.txt\n\tfour\n",
        details(P, "ada", "first"),
    );
    let p = parse(out.as_bytes()).unwrap();
    assert_eq!(p.commits.len(), 1);
    let prev = |path: &str| Some((Oid::from_str(P).unwrap(), path.to_owned()));
    assert_eq!(
        p.origins,
        [
            BlameOrigin { commit: 0, path: "a.txt".into(), previous: prev("old.txt") },
            BlameOrigin { commit: 0, path: "b.txt".into(), previous: prev("b-old.txt") },
        ]
    );
    assert_eq!(p.runs, [run(0, 0, 0, 2), run(0, 1, 2, 1), run(0, 0, 3, 1)]);
    assert_eq!(p.orig_lines, [3, 4, 8, 9], "from the header's second field");
}

#[test]
fn a_commit_without_previous_has_none() {
    let out = format!("{A} 1 1 1\nauthor ada\nsummary first\nboundary\nfilename a.txt\n\tone\n");
    let p = parse(out.as_bytes()).unwrap();
    assert_eq!(p.origins, [BlameOrigin { commit: 0, path: "a.txt".into(), previous: None }]);
}

#[test]
fn quoted_filename_is_unquoted() {
    let out = format!("{A} 1 1 1\nauthor ada\nsummary first\nprevious {P} \"old \\\"x\\\".txt\"\nfilename \"caf\\303\\251.txt\"\n\tone\n");
    let p = parse(out.as_bytes()).unwrap();
    assert_eq!(p.origins[0].path, "café.txt");
    assert_eq!(p.origins[0].previous, Some((Oid::from_str(P).unwrap(), "old \"x\".txt".to_owned())));
}

#[test]
fn missing_filename_on_first_appearance_is_refused() {
    let out = format!("{A} 1 1 1\nauthor ada\nsummary first\n\tone\n");
    assert!(matches!(parse(out.as_bytes()), Err(Error::Decode(_))));
}
