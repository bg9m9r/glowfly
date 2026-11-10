//! The hygiene check that keeps working-tree file work behind its trust gate (see hygiene.rs):
//! in glowfly-model's `update/`, only `file_trust.rs` makes an `Effect::Files` (its `push` holds
//! work that starts git while the repository isn't trusted), and `trust_seal.rs` reads one for
//! the runner's re-check. Whether a file effect starts git is decided per variant in
//! `FilesEffect::runs_git`, an exhaustive match.

use super::all_sources;

const UPDATE: &str = "crates/glowfly-model/src/update/";
const ALLOWED: &[&str] = &["crates/glowfly-model/src/update/file_trust.rs", "crates/glowfly-model/src/update/trust_seal.rs"];

/// Whether `line` names `Effect::Files(` itself (not another type's `…Effect::Files(`).
fn names_file_effect(line: &str) -> bool {
    const NEEDLE: &str = "Effect::Files(";
    line.match_indices(NEEDLE).any(|(i, _)| !line[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
}

#[test]
fn file_effects_are_pushed_through_the_trust_gate() {
    let mut found = Vec::new();
    for s in all_sources().iter().filter(|s| !s.test && s.path.starts_with(UPDATE) && !ALLOWED.contains(&s.path.as_str())) {
        for (n, line) in s.text.lines().enumerate() {
            if !line.trim_start().starts_with("//") && names_file_effect(line) {
                found.push(format!("{}:{}", s.path, n + 1));
            }
        }
    }
    assert!(found.is_empty(), "send file work with update/file_trust.rs's `push`, which asks before git runs: {found:?}");
}

#[test]
fn the_check_tells_a_file_effect_from_another_areas_files() {
    assert!(names_file_effect("fx.push(Effect::Files(FilesEffect::Restore { workdir }));"));
    assert!(names_file_effect("let e = crate::effect::Effect::Files(f);"));
    assert!(!names_file_effect("fx.push(Effect::PullRequest(PrEffect::Files(job)));"));
}
