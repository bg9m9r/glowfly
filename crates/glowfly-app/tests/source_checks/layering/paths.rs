//! Reading Rust paths for the layering check: the `use` statements of a file and the trees they
//! import, and the functions a line calls (`a::b::f(` and bare `f(`). Works on code whose
//! comments and literal text are already blanked (`lexer::bare_lines`).

/// A path bound by a `use`: its segments (a glob ends in `*`), the name it binds and whether
/// the `use` is public (`pub`, `pub(crate)` and the like), so other modules see it.
#[derive(Debug, PartialEq)]
pub struct Import {
    pub path: Vec<String>,
    pub name: String,
    pub public: bool,
    pub line: usize,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every path the `use` statements of `code` import.
pub fn imports(code: &str) -> Vec<Import> {
    let mut out = Vec::new();
    for (line, public, stmt) in use_statements(code) {
        let mut paths = Vec::new();
        use_tree(&tokens(stmt), &mut 0, &[], &mut paths);
        out.extend(paths.into_iter().map(|(path, name)| Import { path, name, public, line }));
    }
    out
}

/// The `use` statements of `code`, as (line it starts on, whether it is public, the text after
/// `use`).
fn use_statements(code: &str) -> Vec<(usize, bool, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    for chunk in code.split_inclusive([';', '{', '}']) {
        let start = at;
        at += chunk.len();
        let Some(k) = chunk.match_indices("use ").map(|(k, _)| k).find(|&k| k == 0 || !chunk[..k].ends_with(is_word)) else { continue };
        let before = chunk[..k].trim_end();
        let public = before.ends_with("pub") || (before.ends_with(')') && before.rsplit_once("pub(").is_some());
        // A use tree with braces spans chunks: read on to its `;`.
        let begin = start + k;
        let end = code[begin..].find(';').map_or(code.len(), |e| begin + e);
        let line = code[..begin].matches('\n').count() + 1;
        out.push((line, public, &code[begin + 4..end]));
    }
    out
}

fn tokens(tree: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in tree.chars() {
        if is_word(c) {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
        if "{},*".contains(c) {
            out.push(c.to_string());
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Expands the use tree at `tokens[*i..]` under `prefix` into (full path, the name it binds).
fn use_tree(tokens: &[String], i: &mut usize, prefix: &[String], out: &mut Vec<(Vec<String>, String)>) {
    let mut path = prefix.to_vec();
    while let Some(t) = tokens.get(*i) {
        match t.as_str() {
            "{" => {
                *i += 1;
                while tokens.get(*i).is_some_and(|t| t != "}") {
                    use_tree(tokens, i, &path, out);
                    if tokens.get(*i).is_some_and(|t| t == ",") {
                        *i += 1;
                    }
                }
                *i += 1;
                return;
            }
            "," | "}" => break,
            "*" => {
                *i += 1;
                out.push((path.iter().cloned().chain(["*".to_owned()]).collect(), "*".to_owned()));
                return;
            }
            "as" => {
                *i += 1;
                if let Some(alias) = tokens.get(*i) {
                    out.push((path.clone(), alias.clone()));
                    *i += 1;
                }
                return;
            }
            "self" => {
                *i += 1;
                let alias = if tokens.get(*i).is_some_and(|t| t == "as") {
                    *i += 2;
                    tokens.get(*i - 1).cloned()
                } else {
                    path.last().cloned()
                };
                if let Some(alias) = alias {
                    out.push((path.clone(), alias));
                }
                return;
            }
            _ => {
                path.push(t.clone());
                *i += 1;
            }
        }
    }
    if path.len() > prefix.len()
        && let Some(last) = path.last()
    {
        out.push((path.clone(), last.clone()));
    }
}

/// Words that a `(` can follow without being a call.
const KEYWORDS: &[&str] = &[
    "pub", "unsafe", "dyn", "if", "while", "match", "return", "in", "for", "loop", "move", "as", "let", "mut", "ref", "where", "fn",
    "impl", "await",
];

/// The functions called on `line` (`a::b::f(`, `f(`), as their segments; method calls
/// (`.f(`), macros (`f!(`), declarations (`fn f(`) and keywords are not.
pub fn call_paths(line: &str) -> Vec<Vec<String>> {
    let b = line.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = Vec::new();
    for (k, _) in line.match_indices('(') {
        // A turbofish (`f::<T>(`): the path ends before it.
        let mut k = k;
        if k > 0 && b[k - 1] == b'>' {
            let mut depth = 0i32;
            let open = (0..k).rev().find(|&j| {
                depth += i32::from(b[j] == b'>') - i32::from(b[j] == b'<');
                depth == 0
            });
            match open {
                Some(j) if j >= 2 && &b[j - 2..j] == b"::" => k = j - 2,
                _ => continue,
            }
        }
        let mut start = k;
        while start > 0 && (word(b[start - 1]) || (start >= 2 && &b[start - 2..start] == b"::")) {
            start -= if word(b[start - 1]) { 1 } else { 2 };
        }
        let path = &line[start..k];
        let before = line[..start].trim_end();
        let declared = before.split(|c: char| !is_word(c)).next_back() == Some("fn");
        if path.is_empty() || line[..start].ends_with(['.', ':']) || declared || path.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let segs: Vec<String> = path.split("::").map(str::to_owned).collect();
        if segs.len() > 1 || !KEYWORDS.contains(&path) {
            out.push(segs);
        }
    }
    out
}

#[test]
fn reads_use_trees_and_calls() {
    let shown: Vec<String> = imports("use glowfly_core::{forge::{self, Host}, repo as r, gitcli::*, askpass::server};\npub use x::y;")
        .iter()
        .map(|i| format!("{}={}{}", i.path.join("::"), i.name, if i.public { " pub" } else { "" }))
        .collect();
    assert_eq!(
        shown,
        [
            "glowfly_core::forge=forge",
            "glowfly_core::forge::Host=Host",
            "glowfly_core::repo=r",
            "glowfly_core::gitcli::*=*",
            "glowfly_core::askpass::server=server",
            "x::y=y pub"
        ]
    );
    let calls: Vec<String> = call_paths(
        "let x = repo::load(a, b::c(d)); s.m(); Type::new(); x.y::<T>(); f(1); g!(2); if (x) {} glowfly_core::askpass::server::start()",
    )
    .iter()
    .map(|p| p.join("::"))
    .collect();
    assert_eq!(calls, ["repo::load", "b::c", "Type::new", "f", "glowfly_core::askpass::server::start"]);
    assert!(call_paths("pub(crate) fn load(p: &Path) -> u8 {").is_empty());
    assert_eq!(call_paths("let p = get_paged::<Vec<T>>(auth, url)?; fn by_link<T: X>(a: u8)"), [vec!["get_paged"]]);
    let stmts: Vec<(usize, bool, &str)> =
        use_statements("fn a() {\n    use glowfly_core::{\n        repo::{self, X},\n    };\n}\npub(crate) use x::y;\n");
    assert_eq!(stmts, [(2, false, "glowfly_core::{\n        repo::{self, X},\n    }"), (6, true, "x::y")]);
}
