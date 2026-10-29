//! The third-party notices (THIRD-PARTY-NOTICES.md), embedded at build time so Help > About can
//! show the licenses of the bundled fonts, syntax definitions and libraries without reading any
//! file. The file's licence texts sit in marked sections, in the order of `SECTIONS`.

use std::sync::OnceLock;

/// The whole notices file.
pub const TEXT: &str = include_str!("../../../THIRD-PARTY-NOTICES.md");

#[cfg(test)]
mod fresh_tests;
#[cfg(test)]
mod generate;
#[cfg(test)]
mod spdx;

/// One marked section of the file.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub begin: &'static str,
    pub end: &'static str,
    /// Whether its `<summary>` lines are source paths, named with `display_name`; otherwise
    /// the summary is the name.
    paths: bool,
}

/// The hand-maintained licenses of the bundled fonts.
pub const FONTS: Section = Section { begin: "<!-- BEGIN FONT LICENSES -->", end: "<!-- END FONT LICENSES -->", paths: true };
/// egui's built-in fonts, generated from the epaint_default_fonts crate.
pub const EGUI_FONTS: Section =
    Section { begin: "<!-- BEGIN GENERATED EGUI FONT LICENSES -->", end: "<!-- END GENERATED EGUI FONT LICENSES -->", paths: false };
/// The syntax licenses, generated from the two-face crate.
pub const SYNTAX: Section =
    Section { begin: "<!-- BEGIN GENERATED SYNTAX LICENSES -->", end: "<!-- END GENERATED SYNTAX LICENSES -->", paths: true };
/// The WHATWG Encoding Standard's tables (hand-maintained).
pub const ENCODING: Section = Section { begin: "<!-- BEGIN ENCODING LICENSES -->", end: "<!-- END ENCODING LICENSES -->", paths: false };
/// libgit2's COPYING and where its source is, generated from the libgit2-sys crate.
pub const LIBGIT2: Section =
    Section { begin: "<!-- BEGIN GENERATED LIBGIT2 LICENSE -->", end: "<!-- END GENERATED LIBGIT2 LICENSE -->", paths: false };
/// The Rust crates compiled in, generated from Cargo.lock.
pub const CRATES: Section =
    Section { begin: "<!-- BEGIN GENERATED CRATE LICENSES -->", end: "<!-- END GENERATED CRATE LICENSES -->", paths: false };
/// The AppImage runtime and the libraries it links (hand-maintained).
pub const APPIMAGE: Section = Section { begin: "<!-- BEGIN APPIMAGE LICENSES -->", end: "<!-- END APPIMAGE LICENSES -->", paths: false };

/// The sections, in file order.
pub const SECTIONS: [Section; 7] = [FONTS, EGUI_FONTS, SYNTAX, ENCODING, LIBGIT2, CRATES, APPIMAGE];

/// One bundled font's, definition's or library's license.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// A readable name, such as "Noto Sans", "Rust" or "Crontab / cron_descriptor".
    pub name: String,
    /// Where the license comes from ("fonts/Noto Sans/OFL.txt", "syntaxes/02_Extra/AWK/LICENSE").
    pub source: String,
    /// The license text.
    pub text: String,
}

impl Notice {
    /// The full list a shortened name stands for (the source, for a block covering many
    /// crates); None when the name says it all.
    pub fn detail(&self) -> Option<&str> {
        (self.name != self.source && !self.source.contains('/')).then_some(self.source.as_str())
    }
}

/// How many crates a block's name lists before "and N more".
const NAMED: usize = 3;

/// A name for a summary that lists what it covers (`MIT: a 1.0, b 2.0, c 1.1, d 0.3`): its
/// first few entries without versions and how many more (`MIT: a, b, c and 1 more`). Other
/// summaries are their own name.
pub fn short_name(summary: &str) -> String {
    let Some((label, list)) = summary.split_once(": ") else { return summary.to_owned() };
    let items: Vec<&str> = list.split(", ").collect();
    if items.len() <= NAMED {
        return summary.to_owned();
    }
    let first: Vec<&str> = items[..NAMED].iter().map(|c| c.split_whitespace().next().unwrap_or(c)).collect();
    format!("{label}: {} and {} more", first.join(", "), items.len() - NAMED)
}

/// The licenses of every section, in `SECTIONS` order. Each `<details>` block gives its
/// `<summary>` (the source) and the lines of its text fence. A section without both its markers
/// gives nothing: it is not as written.
pub fn parse(md: &str) -> Vec<Notice> {
    SECTIONS.iter().flat_map(|s| section(md, s)).collect()
}

/// The `<details>` blocks of section `s`, in file order; empty without its end marker.
pub fn section(md: &str, s: &Section) -> Vec<Notice> {
    let (begin, end) = (s.begin, s.end);
    let mut out = Vec::new();
    let mut inside = false;
    let mut source: Option<String> = None;
    let mut text: Option<Vec<&str>> = None;
    for line in md.lines() {
        if !inside {
            inside = line.trim() == begin;
            continue;
        }
        if line.trim() == end {
            return out;
        }
        if let Some(lines) = text.as_mut() {
            if line.trim_start().starts_with("```") {
                let body = lines.join("\n").trim_end().to_owned();
                if let Some(src) = source.take() {
                    let name = if s.paths { display_name(&src) } else { short_name(&src) };
                    out.push(Notice { name, source: src, text: body });
                }
                text = None;
            } else {
                lines.push(line);
            }
        } else if let Some(rest) = line.trim().strip_prefix("<summary>") {
            source = Some(rest.strip_suffix("</summary>").unwrap_or(rest).trim().to_owned());
        } else if line.trim_start().starts_with("```") && source.is_some() {
            text = Some(Vec::new());
        }
    }
    Vec::new()
}

/// A readable name for a license's source path: the folders below the syntax group (or below
/// `fonts`), joined with " / ", with the package folder's underscores as spaces
/// ("syntaxes/02_Extra/HTML_(Twig)/LICENSE" is "HTML (Twig)", "fonts/Noto Sans/OFL.txt" is
/// "Noto Sans").
pub fn display_name(source: &str) -> String {
    let mut parts: Vec<&str> = source.split('/').filter(|p| !p.is_empty()).collect();
    if parts.first().is_some_and(|p| *p == "syntaxes" || *p == "fonts") {
        parts.remove(0);
    }
    if parts.first().is_some_and(|p| is_group(p)) {
        parts.remove(0);
    }
    if parts.len() > 1 {
        parts.pop();
    }
    // The package folder names the language ("Org_mode"); folders inside it are project names
    // and keep their spelling ("cron_descriptor").
    let mut names: Vec<String> = parts.iter().map(|p| (*p).to_owned()).collect();
    if let Some(first) = names.first_mut() {
        *first = first.replace('_', " ");
    }
    names.join(" / ")
}

/// A group folder such as "01_Packages" or "02_Extra".
fn is_group(part: &str) -> bool {
    part.split_once('_').is_some_and(|(n, rest)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && !rest.is_empty())
}

/// The embedded file's licenses, parsed once.
pub fn licenses() -> &'static [Notice] {
    static CACHE: OnceLock<Vec<Notice>> = OnceLock::new();
    CACHE.get_or_init(|| parse(TEXT))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "intro\n<!-- BEGIN GENERATED SYNTAX LICENSES -->\n<details>\n<summary>syntaxes/01_Packages/Rust/LICENSE.txt</summary>\n\n````text\nMIT terms\n\nline two\n````\n</details>\n\n<details>\n<summary>syntaxes/02_Extra/Crontab/cron_descriptor/LICENSE</summary>\n\n````text\nOther terms\n````\n</details>\n\n<!-- END GENERATED SYNTAX LICENSES -->\n";

    #[test]
    fn the_embedded_file_parses_every_block() {
        let all = parse(TEXT);
        assert_eq!(all.len(), TEXT.matches("<details>").count());
        for n in &all {
            assert!(!n.name.is_empty() && !n.text.is_empty(), "{n:?}");
            assert!(!n.text.contains("```") && !n.text.contains("<summary>"), "{}", n.source);
        }
        let rust = all.iter().find(|n| n.source.ends_with("/Rust/LICENSE.txt")).expect("the Rust license");
        assert_eq!(rust.name, "Rust");
        assert!(rust.text.contains("Permission is hereby granted"));
        assert_eq!(licenses(), all.as_slice());
    }

    #[test]
    fn crlf_parses_like_lf() {
        let lf = parse(SAMPLE);
        assert_eq!(lf.len(), 2);
        assert_eq!(lf[0].text, "MIT terms\n\nline two");
        assert_eq!(lf[1].name, "Crontab / cron_descriptor");
        assert_eq!(parse(&SAMPLE.replace('\n', "\r\n")), lf);
    }

    #[test]
    fn no_markers_no_notices() {
        let bare = SAMPLE.replace(SYNTAX.begin, "").replace(SYNTAX.end, "");
        assert!(parse(&bare).is_empty());
        assert!(parse("").is_empty());
    }

    #[test]
    fn long_lists_are_shortened() {
        assert_eq!(short_name("MIT: a 1.0, b 2.0, c 1.1, d 0.3, e 4"), "MIT: a, b, c and 2 more");
        assert_eq!(short_name("ISC: ring 0.17.14"), "ISC: ring 0.17.14");
        assert_eq!(short_name("WHATWG Encoding Standard (encoding_rs)"), "WHATWG Encoding Standard (encoding_rs)");
        let n = |name: &str, source: &str| Notice { name: name.into(), source: source.into(), text: String::new() };
        assert_eq!(
            n("MIT: a, b, c and 2 more", "MIT: a 1.0, b 2.0, c 1.1, d 0.3, e 4").detail(),
            Some("MIT: a 1.0, b 2.0, c 1.1, d 0.3, e 4")
        );
        assert_eq!(n("Rust", "syntaxes/01_Packages/Rust/LICENSE.txt").detail(), None);
        assert_eq!(n("ISC: ring 0.17.14", "ISC: ring 0.17.14").detail(), None);
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name("syntaxes/01_Packages/Rust/LICENSE.txt"), "Rust");
        assert_eq!(display_name("syntaxes/02_Extra/HTML_(Twig)/LICENSE"), "HTML (Twig)");
        assert_eq!(display_name("syntaxes/02_Extra/Crontab/cron_descriptor/LICENSE"), "Crontab / cron_descriptor");
        assert_eq!(display_name("syntaxes/02_Extra/PureScript/NOTICE"), "PureScript");
        assert_eq!(display_name("syntaxes/02_Extra/MediaWiki/license.md"), "MediaWiki");
        assert_eq!(display_name("syntaxes/02_Extra/Org_mode/LICENSE"), "Org mode");
        assert_eq!(display_name("fonts/Noto Sans/OFL.txt"), "Noto Sans");
        assert_eq!(display_name("fonts/JetBrains Mono/OFL.txt"), "JetBrains Mono");
    }

    #[test]
    fn the_embedded_file_names_the_bundled_fonts() {
        let all = licenses();
        let first_syntax = all.iter().position(|n| n.source.starts_with("syntaxes/")).expect("a syntax license");
        for name in ["Noto Sans", "JetBrains Mono"] {
            let i = all.iter().position(|n| n.name == name).unwrap_or_else(|| panic!("no {name} license"));
            assert!(i < first_syntax, "{name} comes before the syntax licenses");
            assert!(all[i].text.contains("SIL OPEN FONT LICENSE") && all[i].text.contains("Version 1.1"), "{name}");
        }
    }

    /// Font blocks before the syntax sample, as in the file.
    fn with_fonts(font_end: bool, syntax_end: bool) -> String {
        let fonts = "<!-- BEGIN FONT LICENSES -->\n<details>\n<summary>fonts/Noto Sans/OFL.txt</summary>\n\n````text\nFont terms\n````\n</details>\n\n";
        let mut md = format!("intro\n{fonts}{}\n{SAMPLE}", if font_end { FONTS.end } else { "" });
        if !syntax_end {
            md = md.replace(SYNTAX.end, "");
        }
        md
    }

    #[test]
    fn font_section_parses_like_the_syntax_section() {
        let both = parse(&with_fonts(true, true));
        let names: Vec<&str> = both.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["Noto Sans", "Rust", "Crontab / cron_descriptor"]);
        assert_eq!(both[0].text, "Font terms");
        assert_eq!(parse(&with_fonts(true, true).replace('\n', "\r\n")), both);
        // A section missing its END marker gives nothing; the other still parses.
        assert_eq!(parse(&with_fonts(false, true)), both[1..]);
        assert_eq!(parse(&with_fonts(true, false)), both[..1]);
    }
}
