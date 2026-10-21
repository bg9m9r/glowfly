//! Case-insensitive matching for filters and searches that run over many names or commits per
//! frame: no allocation for ASCII text, which is nearly all of it.

/// Whether `hay`, lowercased, contains `needle`, which the caller has already lowercased (once
/// per query, not per item). ASCII text is compared in place; other text is lowercased first,
/// so the result always equals `hay.to_lowercase().contains(needle)`.
pub fn contains_lower(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !hay.is_ascii() {
        return hay.to_lowercase().contains(needle);
    }
    // Lowercased ASCII stays ASCII, so it can't contain anything else.
    if !needle.is_ascii() {
        return false;
    }
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    h.len() >= n.len() && h.windows(n.len()).any(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

/// Whether the lowercase hex spelling of `id` (an object id's bytes) starts with `prefix`,
/// without spelling it out.
pub fn hex_starts_with(id: &[u8], prefix: &str) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let p = prefix.as_bytes();
    p.len() <= id.len() * 2
        && p.iter().enumerate().all(|(i, c)| {
            let byte = id[i / 2];
            let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0xf };
            HEX[nibble as usize] == *c
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_lower_matches_lowercasing_then_contains() {
        let cases = [
            ("Fix the Parser", "parser"),
            ("Fix the Parser", "fix t"),
            ("Fix the Parser", "parsers"),
            ("Fix the Parser", ""),
            ("", "a"),
            ("ÄNDERUNG am Löser", "änderung"),
            ("ÄNDERUNG am Löser", "löser"),
            ("ÄNDERUNG am Löser", "lose"),
            ("plain ascii", "ü"),
            ("feature/ABC-12", "abc-1"),
        ];
        for (hay, needle) in cases {
            assert_eq!(contains_lower(hay, needle), hay.to_lowercase().contains(needle), "{hay:?} / {needle:?}");
        }
    }

    #[test]
    fn hex_prefix_matches_the_spelled_out_id() {
        let id = [0x0a, 0xbc, 0xde, 0xf1];
        let full = "0abcdef1";
        for n in 0..=full.len() {
            assert!(hex_starts_with(&id, &full[..n]));
        }
        assert!(!hex_starts_with(&id, "0abd"));
        assert!(!hex_starts_with(&id, "0ABC"));
        assert!(!hex_starts_with(&id, "0abcdef12"));
        assert!(!hex_starts_with(&id, "zz"));
    }
}
