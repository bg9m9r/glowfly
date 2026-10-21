//! The third-party notices (THIRD-PARTY-NOTICES.md), embedded at build time so Help > About can
//! show the licenses of the bundled fonts and syntax definitions without reading any file.

use std::sync::OnceLock;

/// The whole notices file.
pub const TEXT: &str = include_str!("../../../THIRD-PARTY-NOTICES.md");

/// The hand-maintained font licenses (first in the file).
const FONT_BEGIN: &str = "<!-- BEGIN FONT LICENSES -->";
const FONT_END: &str = "<!-- END FONT LICENSES -->";
/// The syntax licenses, generated from the two-face crate.
const BEGIN: &str = "<!-- BEGIN GENERATED SYNTAX LICENSES -->";
const END: &str = "<!-- END GENERATED SYNTAX LICENSES -->";

/// One bundled font's or definition's license.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// A readable name, such as "Noto Sans", "Rust" or "Crontab / cron_descriptor".
    pub name: String,
    /// Where the license comes from ("fonts/Noto Sans/OFL.txt", "syntaxes/02_Extra/AWK/LICENSE").
    pub source: String,
    /// The license text.
    pub text: String,
}

/// The licenses of the font section, then of the syntax section (their order in the file).
/// Each `<details>` block gives its `<summary>` (the source) and the lines of its text fence. A
/// section without both its markers gives nothing: it is not as written.
pub fn parse(md: &str) -> Vec<Notice> {
    let mut out = section(md, FONT_BEGIN, FONT_END);
    out.extend(section(md, BEGIN, END));
    out
}

/// The `<details>` blocks between `begin` and `end`, in file order; empty without `end`.
fn section(md: &str, begin: &str, end: &str) -> Vec<Notice> {
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
                    out.push(Notice { name: display_name(&src), source: src, text: body });
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
        let bare = SAMPLE.replace(BEGIN, "").replace(END, "");
        assert!(parse(&bare).is_empty());
        assert!(parse("").is_empty());
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
        let mut md = format!("intro\n{fonts}{}\n{SAMPLE}", if font_end { FONT_END } else { "" });
        if !syntax_end {
            md = md.replace(END, "");
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
