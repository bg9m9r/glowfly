//! The hygiene check that keeps `Effect` free of flat variants (see hygiene.rs, #373): every
//! variant is an effect area's, one row in glowfly-model's `effect/areas.rs` carrying the area's
//! own effect type with its `EffectMeta` and an `Execute` impl in `runner/<area>.rs`. The flat
//! variants that predated the areas are all gone; this keeps a new one from coming back, either
//! in the `effect_enum!` call in `effect/mod.rs` or in the enum the macro declares.

use super::all_sources;

/// The file that declares `Effect`, and the call that must declare it.
const EFFECT_MOD: &str = "crates/glowfly-model/src/effect/mod.rs";
const EFFECT_CALL: &str = "effect_areas!{effect_enum!{}}";

/// The file with `effect_enum!`, and the only body its `Effect` may have: the area rows.
const AREAS: &str = "crates/glowfly-model/src/effect/areas.rs";
const ENUM_OPEN: &str = "pub enum Effect {";
const AREA_ROWS: &str = "$($(#[$doc])*$V($T),)*";

fn squeezed(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The body of the first `pub enum Effect { … }` in `src`, without whitespace; None without one.
fn enum_body(src: &str) -> Option<String> {
    let start = src.find(ENUM_OPEN)? + ENUM_OPEN.len();
    let mut depth = 1usize;
    for (i, c) in src[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 1 => return Some(squeezed(&src[start..start + i])),
            '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn source(path: &str) -> String {
    all_sources().into_iter().find(|s| s.path == path).unwrap_or_else(|| panic!("{path}")).text
}

#[test]
fn effect_has_no_flat_variants() {
    const HOW: &str = "A new effect goes in an area: a variant of the area's effect type in effect/<area>.rs with its EffectMeta, \
                       an Execute impl in runner/<area>.rs and, for a new area, a row in effect/areas.rs.";
    assert!(
        squeezed(&source(EFFECT_MOD)).contains(EFFECT_CALL),
        "{EFFECT_MOD} declares Effect with `effect_areas! {{ effect_enum! {{}} }}` only. {HOW}"
    );
    let body = enum_body(&source(AREAS));
    assert_eq!(body.as_deref(), Some(AREA_ROWS), "effect_enum! in {AREAS} declares only the area rows. {HOW}");
}

#[test]
fn the_check_sees_a_variant_beside_the_rows() {
    let rows = "macro_rules! effect_enum {\n    ({} $( $V:ident($T:ty), )*) => {\n        pub enum Effect {\n            $( $(#[$doc])* $V($T), )*\n        }\n    };\n}\n";
    assert_eq!(enum_body(rows).as_deref(), Some(AREA_ROWS));
    let flat = rows.replace("$V($T), )*\n", "$V($T), )*\n            Forge { cwd: PathBuf },\n");
    assert_ne!(enum_body(&flat).as_deref(), Some(AREA_ROWS), "a flat struct variant is seen");
    assert_eq!(enum_body("enum Other { A }"), None);
}
