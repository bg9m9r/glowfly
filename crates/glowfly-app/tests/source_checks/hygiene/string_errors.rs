//! The hygiene ratchet on English-string errors in result messages (see hygiene.rs, #367): a
//! `Result<_, String>` in glowfly-model's `msg/` carries an error the runner already worded in
//! English, so `update` can neither tell the causes apart nor translate them. Such fields number
//! at most `STRING_ERRORS`, which only goes down: convert an area's results to a typed error
//! (worded in `state/error_words/typed.rs` with `t.tr`), then lower it.

use super::all_sources;
use crate::lexer::bare_lines;

/// How many `Result<_, String>` types the message enums under `msg/` still hold. Lower it when
/// you convert an area to a typed error; never raise it.
const STRING_ERRORS: usize = 49;

/// The folder of the message enums.
const MSG_DIR: &str = "crates/glowfly-model/src/msg/";

/// The lines (1-based) of `src`'s code with a `Result<…>` whose error type is `String`, once per
/// `Result`. Comments, literals and inline test modules are skipped.
fn string_errors(src: &str) -> Vec<usize> {
    let code = bare_lines(src).join("\n");
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(off) = code[at..].find("Result<") {
        let start = at + off;
        at = start + "Result<".len();
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        if error_type(&code[at..]).is_some_and(|e| e == "String" || e == "std::string::String") {
            found.push(code[..start].matches('\n').count() + 1);
        }
    }
    found
}

/// The last top-level argument of the generic list that `rest` opens (just after its `<`),
/// trimmed: `Vec<u8>, String> …` gives `String`. None when the list doesn't close.
fn error_type(rest: &str) -> Option<&str> {
    let (mut depth, mut last) = (0usize, 0usize);
    for (i, c) in rest.char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' if depth > 0 => depth -= 1,
            '>' => return Some(rest[last..i].trim()),
            ',' if depth == 0 => last = i + 1,
            _ => {}
        }
    }
    None
}

#[test]
fn result_messages_carry_typed_errors() {
    let mut sites = Vec::new();
    for s in all_sources().iter().filter(|s| !s.test && s.path.starts_with(MSG_DIR)) {
        sites.extend(string_errors(&s.text).into_iter().map(|line| format!("{}:{line}", s.path)));
    }
    assert!(
        sites.len() <= STRING_ERRORS,
        "{} Result<_, String> types in {MSG_DIR}, more than STRING_ERRORS ({STRING_ERRORS}). Carry a typed error instead: an enum \
         in glowfly-core or glowfly-model, worded in crates/glowfly-model/src/state/error_words/typed.rs with t.tr. The types:\n{}",
        sites.len(),
        sites.join("\n")
    );
    assert!(
        sites.len() == STRING_ERRORS,
        "only {} Result<_, String> types in {MSG_DIR} now: lower STRING_ERRORS in tests/source_checks/hygiene/string_errors.rs to {}",
        sites.len(),
        sites.len()
    );
}

#[test]
fn the_string_error_check_catches_what_it_should() {
    let src = "pub enum M {\n    A { result: Result<Vec<u8>, String> },\n    B(Result<(), CredentialError>),\n    \
               C { r: Result<HashMap<String, Vec<String>>, std::string::String> },\n    // D(Result<(), String>),\n    \
               E(MyResult<(), String>),\n    F(Result<Option<(u8, String)>, Box<Error>>),\n    G(Vec<(PathBuf, Result<Scanned, \
               String>)>),\n}\n";
    assert_eq!(string_errors(src), [2, 4, 8], "String errors only, not in comments, other types or a String in the Ok side");
    assert_eq!(error_type("u8, String> x"), Some("String"));
    assert_eq!(error_type("Vec<(u8, String)>, Error>"), Some("Error"));
    assert_eq!(error_type("u8, String"), None);
}
