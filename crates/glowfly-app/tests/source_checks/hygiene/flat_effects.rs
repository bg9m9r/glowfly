//! The hygiene ratchet on `Effect`'s flat variants (see hygiene.rs, #373): the variants declared
//! straight in glowfly-model's `effect/mod.rs` predate the effect areas, and each is spread across
//! `effect/names.rs`, `scope.rs`, `sandbox.rs` and the runner's `execute.rs`. They number at most
//! `FLAT_EFFECTS`, which only goes down: move a group into an area (a variant of the area's effect
//! type in `effect/<area>.rs` with its `EffectMeta`, an `Execute` impl in `runner/<area>.rs` and
//! one row in `effect/areas.rs`), then lower it.

use super::all_sources;
use crate::lexer::bare_lines;

/// How many flat variants `Effect` still declares outside the areas. Lower it when you move some
/// into an area; never raise it.
const FLAT_EFFECTS: usize = 6;

/// The file that declares `Effect`.
const EFFECT_MOD: &str = "crates/glowfly-model/src/effect/mod.rs";

/// The opening of the flat variants' block in `effect/mod.rs`.
const FLAT_BLOCK: &str = "effect_enum! {";

/// The names of the variants in the block `src` opens with `FLAT_BLOCK`, in order: each first
/// identifier at the block's top level, after its opening or a top-level comma. Comments and
/// literals are skipped, and the `>` of a `->` in a field's type closes nothing. Empty when there
/// is no such block.
fn flat_variants(src: &str) -> Vec<String> {
    let code = bare_lines(src).join("\n");
    let Some(open) = code.find(FLAT_BLOCK) else { return Vec::new() };
    let (mut depth, mut want, mut word, mut found, mut prev) = (0usize, true, String::new(), Vec::new(), ' ');
    for c in code[open + FLAT_BLOCK.len()..].chars() {
        let arrow = prev == '-' && c == '>';
        prev = c;
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if want && depth == 0 && !word.is_empty() {
            found.push(std::mem::take(&mut word));
            want = false;
        }
        word.clear();
        match c {
            _ if arrow => {}
            '{' | '(' | '[' | '<' => depth += 1,
            '}' | ')' | ']' | '>' if depth == 0 => break,
            '}' | ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => want = true,
            _ => {}
        }
    }
    found
}

#[test]
fn flat_effects_only_shrink() {
    let sources = all_sources();
    let src = sources.iter().find(|s| s.path == EFFECT_MOD).expect("effect/mod.rs");
    let flat = flat_variants(&src.text);
    assert!(
        flat.len() <= FLAT_EFFECTS,
        "{} flat Effect variants in {EFFECT_MOD}, more than FLAT_EFFECTS ({FLAT_EFFECTS}). A new effect goes in an area: a variant \
         of the area's effect type in effect/<area>.rs with its EffectMeta, an Execute impl in runner/<area>.rs and a row in \
         effect/areas.rs. The flat variants:\n{}",
        flat.len(),
        flat.join("\n")
    );
    assert!(
        flat.len() == FLAT_EFFECTS,
        "only {} flat Effect variants in {EFFECT_MOD} now: lower FLAT_EFFECTS in tests/source_checks/hygiene/flat_effects.rs to {}",
        flat.len(),
        flat.len()
    );
}

#[test]
fn the_flat_effect_count_finds_each_variant_once() {
    let src = "effect_areas! { effect_enum! {\n    /// A doc, with a comma.\n    A {\n        path: PathBuf,\n        \
               pair: Option<(PathBuf, Vec<u8>)>,\n    },\n    // C { x: u8 },\n    B(Vec<String>),\n    D(Box<HashMap<String, \
               u8>>),\n    E {\n        f: bool,\n    },\n} }\nfn after() {}\n";
    assert_eq!(flat_variants(src), ["A", "B", "D", "E"], "variants only, not their fields, types or comments");
    assert!(flat_variants("pub enum Effect { A, B }").is_empty(), "no flat block, nothing counted");
    let arrows = "effect_enum! {\n    A {\n        f: Box<dyn Fn(u8) -> u8 + Send>,\n    },\n    B(fn() -> bool),\n    C,\n}\n";
    assert_eq!(flat_variants(arrows), ["A", "B", "C"], "a `->` in a field's type doesn't end the count");
}
