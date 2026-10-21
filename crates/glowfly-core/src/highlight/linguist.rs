//! Language overrides: names from a `.gitattributes` `linguist-language` value mapped to the
//! bundled syntaxes, and the `SyntaxHint` the highlighter takes.

use super::syntaxes::SYNTAXES;
use syntect::parsing::SyntaxReference;

/// What to highlight a file as: its path, and the language its gitattributes name, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyntaxHint<'a> {
    /// Repo-relative, '/'-separated.
    pub path: &'a str,
    /// A `linguist-language` value, raw (any case, spaces, hyphens or underscores).
    pub language: Option<&'a str>,
}

impl<'a> From<&'a str> for SyntaxHint<'a> {
    fn from(path: &'a str) -> Self {
        Self { path, language: None }
    }
}

/// For paths taken from a slice (`for path in &["a.rs", ..]`).
impl<'a> From<&&'a str> for SyntaxHint<'a> {
    fn from(path: &&'a str) -> Self {
        Self::from(*path)
    }
}

/// Language names and aliases (normalized with `norm`) that differ from a bundled syntax's name,
/// and the bundled syntax each stands for.
pub(super) const ALIASES: &[(&[&str], &str)] = &[
    (&["shell", "shell-script", "sh", "bash", "zsh", "ash"], "Bourne Again Shell (bash)"),
    (&["batchfile", "batch", "bat", "dosbatch", "winbatch"], "Batch File"),
    (&["pwsh", "posh"], "PowerShell"),
    (&["cpp"], "C++"),
    (&["csharp", "cake"], "C#"),
    (&["fsharp"], "F#"),
    (&["objc", "obj-c", "objectivec"], "Objective-C"),
    (&["objc++", "objectivec++"], "Objective-C++"),
    (&["node", "ecmascript"], "JavaScript"),
    (&["tsx", "jsx"], "TypeScriptReact"),
    (&["json-with-comments", "jsonc", "json5"], "JSON"),
    (&["python3"], "Python"),
    (&["vim-script", "vimscript", "vim", "nvim", "viml"], "VimL"),
    (&["emacs-lisp", "elisp", "common-lisp", "scheme"], "Lisp"),
    (&["assembly", "asm", "nasm", "x86-assembly"], "x86_64 Assembly"),
    (&["containerfile"], "Dockerfile"),
    (&["go-module"], "Gomod"),
    (&["go-checksums"], "Gosum"),
    (&["ignore-list", "gitignore"], "Git Ignore"),
    (&["editorconfig", "dosini"], "INI"),
    (&["apacheconf", "apache", "aconf"], "Apache Conf"),
    (&["plsql", "plpgsql", "tsql", "sqlpl"], "SQL"),
    (&["hcl"], "Terraform"),
    (&["html+erb", "erb", "rhtml"], "HTML (Rails)"),
    (&["html+eex", "eex"], "HTML (EEx)"),
    (&["vue"], "Vue Component"),
    (&["asciidoc"], "AsciiDoc (Asciidoctor)"),
    (&["roff", "groff", "troff"], "Groff/troff"),
    (&["graphviz"], "Graphviz (DOT)"),
    (&["protocol-buffer-text-format"], "Protocol Buffer (TEXT)"),
    (&["text", "plain-text", "txt"], "Plain Text"),
];

/// A language name compared loosely: trimmed, ASCII lowercase, with spaces and underscores read
/// as hyphens ("Vim Script" and "vim_script" both give "vim-script").
pub(super) fn norm(s: &str) -> String {
    s.trim().chars().map(|c| if c == ' ' || c == '_' { '-' } else { c.to_ascii_lowercase() }).collect()
}

/// The bundled syntax a `linguist-language` value names: the alias table, then a non-hidden
/// syntax of that name, then the value as an extension. Plain Text for `Text`; `None` when
/// unknown.
pub(super) fn by_language(name: &str) -> Option<&'static SyntaxReference> {
    let key = norm(name);
    if key.is_empty() {
        return None;
    }
    let set = &SYNTAXES.set;
    if let Some((_, target)) = ALIASES.iter().find(|(keys, _)| keys.contains(&key.as_str())) {
        return set.find_syntax_by_name(target);
    }
    set.syntaxes().iter().find(|s| !s.hidden && norm(&s.name) == key).or_else(|| set.find_syntax_by_extension(name.trim()))
}
