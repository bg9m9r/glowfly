//! Release notes in the Markdown that release-please writes to `CHANGELOG.md` (and that GitHub
//! release bodies carry): a `## ` heading per release, `### ` sections, and `* ` items with
//! `**bold**`, `` `code` `` and `[text](url)` markup. Everything else is ignored.

/// One release's notes.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseNotes {
    /// The version as written, such as `0.2.0`.
    pub version: String,
    pub date: Option<String>,
    /// The heading's link (the compare page), when it is https.
    pub url: Option<String>,
    pub sections: Vec<NoteSection>,
}

/// A section such as Features or Bug fixes.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteSection {
    pub title: String,
    /// The breaking changes section.
    pub breaking: bool,
    /// Its items, each a run of styled text.
    pub items: Vec<Vec<Span>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: SpanStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpanStyle {
    Plain,
    Strong,
    Code,
    /// A link to this https address.
    Link(String),
}

/// `## [0.2.0](https://…/compare/v0.1.0...v0.2.0) (2026-10-20)` or `## 0.1.0 (2026-10-01)`.
fn release_heading(rest: &str) -> ReleaseNotes {
    let rest = rest.trim();
    let (version, url, after) = match rest.strip_prefix('[').and_then(|r| r.split_once("](")) {
        Some((v, r)) => match r.split_once(')') {
            Some((u, after)) => (v.to_owned(), https(u), after),
            None => (v.to_owned(), None, ""),
        },
        None => {
            let (v, after) = rest.split_once(' ').unwrap_or((rest, ""));
            (v.to_owned(), None, after)
        }
    };
    let date = after.trim().strip_prefix('(').and_then(|d| d.strip_suffix(')')).map(str::to_owned).filter(|d| !d.is_empty());
    ReleaseNotes { version: version.trim_start_matches(['v', 'V']).to_owned(), date, url, sections: Vec::new() }
}

fn https(url: &str) -> Option<String> {
    crate::forge::https_host(url).map(|_| url.to_owned())
}

/// A section title without leading symbols (`⚠ BREAKING CHANGES` becomes "Breaking changes").
fn section(rest: &str) -> NoteSection {
    let title = rest.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
    let breaking = title.eq_ignore_ascii_case("breaking changes");
    let title = if breaking { "Breaking changes".to_owned() } else { title.to_owned() };
    NoteSection { title, breaking, items: Vec::new() }
}

/// Inline markup of one item.
pub fn spans(text: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    let flush = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span { text: std::mem::take(plain), style: SpanStyle::Plain });
        }
    };
    while let Some(c) = rest.chars().next() {
        if let Some(r) = rest.strip_prefix("**")
            && let Some((inner, after)) = r.split_once("**")
            && !inner.is_empty()
        {
            flush(&mut plain, &mut out);
            out.push(Span { text: inner.to_owned(), style: SpanStyle::Strong });
            rest = after;
        } else if let Some(r) = rest.strip_prefix('`')
            && let Some((inner, after)) = r.split_once('`')
            && !inner.is_empty()
        {
            flush(&mut plain, &mut out);
            out.push(Span { text: inner.to_owned(), style: SpanStyle::Code });
            rest = after;
        } else if let Some(r) = rest.strip_prefix('[')
            && let Some((label, r)) = r.split_once("](")
            && let Some((url, after)) = r.split_once(')')
            && !label.contains(']')
        {
            match https(url) {
                Some(u) => {
                    flush(&mut plain, &mut out);
                    out.push(Span { text: label.to_owned(), style: SpanStyle::Link(u) });
                }
                None => plain.push_str(label),
            }
            rest = after;
        } else {
            plain.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    flush(&mut plain, &mut out);
    out
}

/// Every release in `text`, in the order written (newest first in a changelog).
pub fn parse(text: &str) -> Vec<ReleaseNotes> {
    let mut releases: Vec<ReleaseNotes> = Vec::new();
    // The item being read, until a line that doesn't continue it.
    let mut item: Option<String> = None;
    let finish = |item: &mut Option<String>, releases: &mut Vec<ReleaseNotes>| {
        // Items before any section go into an untitled one.
        if let Some(text) = item.take()
            && let Some(r) = releases.last_mut()
        {
            if r.sections.is_empty() {
                r.sections.push(NoteSection { title: String::new(), breaking: false, items: Vec::new() });
            }
            if let Some(sec) = r.sections.last_mut() {
                sec.items.push(spans(text.trim()));
            }
        }
    };
    for raw in text.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim_start();
        let indented = line.len() > trimmed.len();
        if let Some(rest) = line.strip_prefix("## ") {
            finish(&mut item, &mut releases);
            releases.push(release_heading(rest));
        } else if let Some(rest) = line.strip_prefix("### ") {
            finish(&mut item, &mut releases);
            if let Some(r) = releases.last_mut() {
                r.sections.push(section(rest));
            }
        } else if let Some(rest) = line.strip_prefix("* ").or_else(|| line.strip_prefix("- ")) {
            finish(&mut item, &mut releases);
            item = Some(rest.to_owned());
        } else if indented && !trimmed.is_empty() && item.is_some() {
            if let Some(text) = item.as_mut() {
                text.push(' ');
                text.push_str(trimmed.trim_start_matches(['*', '-']).trim_start());
            }
        } else {
            finish(&mut item, &mut releases);
        }
    }
    finish(&mut item, &mut releases);
    releases
}

/// A GitHub release body's notes for `tag`: the body's release of that version when it is in
/// changelog shape, else the whole body as one release named after the tag (without its `v`),
/// its `## ` headings read as sections.
pub fn parse_body(tag: &str, body: &str) -> ReleaseNotes {
    let version = tag.trim_start_matches(['v', 'V']);
    if let Some(r) = parse(body).into_iter().find(|r| r.version == version) {
        return r;
    }
    let lines: Vec<String> = body.lines().map(|l| l.strip_prefix("## ").map_or_else(|| l.to_owned(), |r| format!("### {r}"))).collect();
    let text = format!("## {version}\n{}", lines.join("\n"));
    let mut notes = parse(&text).into_iter().next().unwrap_or_else(|| release_heading(version));
    notes.version = version.to_owned();
    notes
}

/// Whether `releases` has notes for `version` (written with or without a `v`).
pub fn has_version(releases: &[ReleaseNotes], version: &str) -> bool {
    let want = version.trim_start_matches(['v', 'V']);
    releases.iter().any(|r| r.version == want)
}

#[cfg(test)]
#[path = "changelog_tests.rs"]
mod tests;
