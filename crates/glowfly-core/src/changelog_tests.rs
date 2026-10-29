use super::*;

/// A changelog shaped like release-please's: a linked heading, a plain first-release heading,
/// the three section kinds and items with a scope, code, a commit link and a continuation line.
const FIXTURE: &str = "# Changelog

## [0.2.0](https://github.com/bg9m9r/glowfly/compare/v0.1.0...v0.2.0) (2026-10-20)


### ⚠ BREAKING CHANGES

* **settings:** drop the old `theme.json` format ([abc1234](https://github.com/bg9m9r/glowfly/commit/abc1234))

### Features

* **diff:** split view for wide windows ([def5678](https://github.com/bg9m9r/glowfly/commit/def5678))
* blame view, with a long description that
  continues on the next line ([0a1b2c3](http://example.com/commit/0a1b2c3))

### Bug Fixes

* keep the selection after a refresh

## 0.1.0 (2026-10-01)


### Features

* first release

Some stray text that isn't an item.
";

fn plain(t: &str) -> Span {
    Span { text: t.to_owned(), style: SpanStyle::Plain }
}

#[test]
fn a_release_please_changelog_is_parsed() {
    let notes = parse(FIXTURE);
    assert_eq!(notes.len(), 2);
    let r = &notes[0];
    assert_eq!(r.version, "0.2.0");
    assert_eq!(r.date.as_deref(), Some("2026-10-20"));
    assert_eq!(r.url.as_deref(), Some("https://github.com/bg9m9r/glowfly/compare/v0.1.0...v0.2.0"));
    let titles: Vec<(&str, bool)> = r.sections.iter().map(|s| (s.title.as_str(), s.breaking)).collect();
    assert_eq!(titles, [("Breaking changes", true), ("Features", false), ("Bug Fixes", false)]);
    assert_eq!(
        r.sections[0].items[0],
        vec![
            Span { text: "settings:".into(), style: SpanStyle::Strong },
            plain(" drop the old "),
            Span { text: "theme.json".into(), style: SpanStyle::Code },
            plain(" format ("),
            Span { text: "abc1234".into(), style: SpanStyle::Link("https://github.com/bg9m9r/glowfly/commit/abc1234".into()) },
            plain(")"),
        ]
    );
    // The continuation joins the item, and a link that isn't https stays as plain text.
    let blame = &r.sections[1].items[1];
    let text: String = blame.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(text, "blame view, with a long description that continues on the next line (0a1b2c3)");
    assert!(blame.iter().all(|s| s.style == SpanStyle::Plain), "{blame:?}");
    assert_eq!(r.sections[2].items, vec![vec![plain("keep the selection after a refresh")]]);

    let first = &notes[1];
    assert_eq!((first.version.as_str(), first.date.as_deref(), first.url.as_deref()), ("0.1.0", Some("2026-10-01"), None));
    assert_eq!(first.sections.len(), 1);
    assert_eq!(first.sections[0].items.len(), 1, "stray text is ignored");
}

#[test]
fn edge_cases_of_the_markup() {
    assert!(parse("").is_empty());
    assert!(parse("# Changelog\n\nNothing yet.\n").is_empty());
    assert_eq!(parse(&FIXTURE.replace('\n', "\r\n")), parse(FIXTURE), "CRLF");
    assert_eq!(spans("a ** b"), vec![plain("a ** b")], "an unclosed marker is text");
    assert_eq!(spans("[x](javascript:alert(1))"), vec![plain("x)")], "only https links are links");
    assert!(has_version(&parse(FIXTURE), "0.2.0") && has_version(&parse(FIXTURE), "v0.1.0"));
    assert!(!has_version(&parse(FIXTURE), "0.3.0"));
}

#[test]
fn release_bodies_are_read() {
    let body = "### Features\n\n* **ui:** a thing\n";
    let n = parse_body("v0.3.0", body);
    assert_eq!((n.version.as_str(), n.sections.len()), ("0.3.0", 1));
    assert_eq!(n.sections[0].title, "Features");
    // release-please's own body has the heading.
    let n = parse_body("v0.2.0", FIXTURE);
    assert_eq!((n.version.as_str(), n.date.as_deref()), ("0.2.0", Some("2026-10-20")));
    // GitHub's generated notes: its headings become sections, loose items an untitled one.
    let n = parse_body("v0.3.0", "## What's Changed\n* Fix a thing by @ada in https://github.com/o/r/pull/1\n\n**Full Changelog**: x");
    assert_eq!(n.version, "0.3.0");
    assert_eq!(n.sections[0].title, "What's Changed");
    assert_eq!(n.sections[0].items.len(), 1);
    let n = parse_body("v0.3.0", "* one\n* two\n");
    assert_eq!((n.sections[0].title.as_str(), n.sections[0].items.len()), ("", 2));
    assert!(parse_body("v0.3.0", "").sections.is_empty());
}

/// The fixed header the Release workflow puts above each release's notes
/// (`scripts/release-notes-header.sh`).
const RELEASE_HEADER: &str = include_str!("../../../packaging/release-notes-header.md");

#[test]
fn the_release_notes_header_is_left_out_of_the_notes() {
    let header = RELEASE_HEADER.replace("\r\n", "\n");
    for line in header.lines() {
        assert!(!line.starts_with("## ") && !line.starts_with("### "), "a heading in the header: {line}");
        assert!(!line.starts_with("* ") && !line.starts_with("- "), "a list item in the header: {line}");
    }
    let generated = "## What's Changed\n* Fix a thing by @ada in https://github.com/o/r/pull/1\n";
    for (tag, body) in [("v0.2.0", FIXTURE), ("v0.3.0", generated), ("v0.3.0", "* one\n")] {
        assert_eq!(parse_body(tag, &format!("{header}\n{body}")), parse_body(tag, body), "{tag}");
    }
    assert!(parse_body("v0.3.0", &header).sections.is_empty());
}
