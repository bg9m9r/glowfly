//! The version a new release or hotfix branch is named after, from the existing version tags.

use super::FlowKind;

/// The next version after the highest tag `<tag_prefix>X.Y[.Z]` (all digits): X.(Y+1).0 for a
/// release, X.Y.(Z+1) for a hotfix. None for a feature, or when no tag is a version.
pub fn suggest_version(tags: &[&str], tag_prefix: &str, kind: FlowKind) -> Option<String> {
    if !kind.versioned() {
        return None;
    }
    let (major, minor, patch) = tags.iter().filter_map(|t| parse(t.strip_prefix(tag_prefix)?)).max()?;
    Some(match kind {
        FlowKind::Release => format!("{major}.{}.0", minor.saturating_add(1)),
        _ => format!("{major}.{minor}.{}", patch.saturating_add(1)),
    })
}

/// "X.Y" or "X.Y.Z", each part all digits.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = v.split('.').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let mut n = [0_u64; 3];
    for (slot, p) in n.iter_mut().zip(&parts) {
        if p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        *slot = p.parse().ok()?;
    }
    Some((n[0], n[1], n[2]))
}
