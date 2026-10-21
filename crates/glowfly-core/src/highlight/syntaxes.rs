//! The bundled syntax definitions, the scope-to-kind rules and language detection.
//!
//! The set is dumped at build time by `build.rs`: the curated two-face set (syntect's defaults
//! plus TOML, TypeScript, Dockerfile, Kotlin, Swift and many more) merged with Trellis's own
//! grammars in `crates/glowfly-core/syntaxes/` (PowerShell). Loading the dump takes under 1 ms;
//! regexes compile lazily, the first time a language is parsed.
//!
//! Detection: a language override from gitattributes comes first (`linguist`), then the file's
//! name, then its first line. `.jsx` uses the TSX grammar, as the JavaScript (Babel) grammar
//! doesn't compile under fancy-regex.

use super::TokenKind;
use super::linguist::{SyntaxHint, by_language};
use std::sync::LazyLock;
use syntect::parsing::{Scope, SyntaxReference, SyntaxSet};

/// The syntax set written by `build.rs`.
static DUMP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/syntaxes.packdump"));

/// Immutable data loaded once on first use: the bundled syntax definitions and the
/// scope-to-kind rules.
pub(super) struct Syntaxes {
    pub(super) set: SyntaxSet,
    /// Sorted most specific (longest) scope first, so the first rule that is a prefix of a
    /// scope is the most specific one, whatever the order of the table below.
    pub(super) rules: Vec<(Scope, TokenKind)>,
}

pub(super) static SYNTAXES: LazyLock<Syntaxes> = LazyLock::new(|| {
    use TokenKind::*;
    let table: &[(&[&str], TokenKind)] = &[
        (&["comment"], Comment),
        (&["string", "markup.raw"], String),
        (&["constant.numeric"], Number),
        (&["constant", "support.constant", "variable.language", "keyword.constant"], Constant),
        (&["keyword", "storage", "entity.name.tag", "markup.heading"], Keyword),
        (
            &[
                "entity.name.type",
                "entity.name.class",
                "entity.name.struct",
                "entity.name.enum",
                "entity.name.trait",
                "entity.name.union",
                "entity.name.interface",
                "entity.name.table",
                "entity.name.section",
                "entity.other.inherited-class",
                "support.type",
                "support.class",
                "storage.type.buildin",
            ],
            Type,
        ),
        (&["entity.name.function", "entity.type.function", "support.function", "variable.function"], Function),
        (&["entity.other.attribute-name", "meta.annotation", "storage.modifier.attribute"], Attribute),
    ];
    let mut rules: Vec<_> =
        table.iter().flat_map(|(scopes, kind)| scopes.iter().map(move |s| (Scope::new(s).expect("valid scope"), *kind))).collect();
    // Stable, so equally long scopes keep the table's order.
    rules.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    let set = syntect::dumps::from_uncompressed_data(DUMP).expect("the syntax dump written by build.rs loads");
    Syntaxes { set, rules }
});

/// See `syntax_name`.
pub(super) fn find_syntax(file: SyntaxHint<'_>, first_line: Option<&str>) -> Option<&'static SyntaxReference> {
    let set = &SYNTAXES.set;
    let path = file.path;
    let name = path.rsplit('/').next().unwrap_or(path);
    let found = file.language.and_then(by_language).or_else(|| {
        set.find_syntax_by_extension(name)
            .or_else(|| name.rsplit_once('.').and_then(|(_, ext)| set.find_syntax_by_extension(ext)))
            .or_else(|| alias(name))
            .or_else(|| first_line.and_then(|l| set.find_syntax_by_first_line(l)))
    })?;
    (found.name != "Plain Text").then_some(found)
}

/// Files the bundled set doesn't name: `Dockerfile` in any case, `Dockerfile.dev`,
/// `Containerfile` and `Containerfile.prod`; and `.jsx`, highlighted with the TSX grammar (the
/// JavaScript (Babel) grammar doesn't compile under fancy-regex, so it isn't bundled).
fn alias(name: &str) -> Option<&'static SyntaxReference> {
    let lower = name.to_ascii_lowercase();
    if lower.rsplit_once('.').is_some_and(|(_, ext)| ext == "jsx") {
        return SYNTAXES.set.find_syntax_by_name("TypeScriptReact");
    }
    let docker = ["dockerfile", "containerfile"].iter().any(|n| lower == *n || lower.strip_prefix(n).is_some_and(|r| r.starts_with('.')));
    docker.then(|| SYNTAXES.set.find_syntax_by_name("Dockerfile")).flatten()
}

/// The language for a file: a `linguist-language` override (`SyntaxHint::language`) first,
/// when it names a bundled syntax (see `linguist`); else by whole file name (`Makefile`,
/// `Dockerfile`, `CMakeLists.txt`), then by extension, then container-file names
/// (`Containerfile`, `Dockerfile.dev`) and `.jsx` (the TSX grammar), then by the file's first
/// line (shebangs, `FROM image`). Never touches the disk. `None` for unknown or plain text (so a
/// `Text` override shows the file plain). Takes a path (`&str`) or a `SyntaxHint`.
pub fn syntax_name<'a>(file: impl Into<SyntaxHint<'a>>, first_line: Option<&str>) -> Option<&'static str> {
    find_syntax(file.into(), first_line).map(|s| s.name.as_str())
}
