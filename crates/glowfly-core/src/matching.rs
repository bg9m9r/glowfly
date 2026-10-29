//! Case-insensitive matching for filters and searches that run over many names or commits per
//! frame: no allocation for an ASCII needle, which is nearly every query, even in text that
//! isn't all ASCII (a whole commit message often has a dash, a quote or a name that isn't).

/// The only characters outside ASCII whose lowercase holds ASCII: İ (to "i" and a combining dot)
/// and the Kelvin sign (to "k"). A test checks every character.
const LOWER_TO_ASCII: [char; 2] = ['\u{130}', '\u{212A}'];

/// Whether `hay`, lowercased, contains `needle`, which the caller has already lowercased (once
/// per query, not per item). An ASCII needle is looked for in place, ignoring ASCII case: other
/// characters lowercase to other characters (but for `LOWER_TO_ASCII`, which send the haystack
/// the slow way) and their bytes are never ASCII, so no match can start or end inside one. A
/// needle that isn't ASCII lowercases the haystack first. The result always equals
/// `hay.to_lowercase().contains(needle)`.
pub fn contains_lower(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let ascii = hay.is_ascii();
    if !needle.is_ascii() {
        // Lowercased ASCII stays ASCII, so it can't contain anything else.
        return !ascii && hay.to_lowercase().contains(needle);
    }
    if !ascii && hay.contains(LOWER_TO_ASCII) {
        return hay.to_lowercase().contains(needle);
    }
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    let Some(last) = h.len().checked_sub(n.len()) else { return false };
    // The places the needle's first byte starts, in either case (a SIMD scan), each compared
    // with the rest of the needle ignoring ASCII case.
    let (first, rest) = (n[0], &n[1..]);
    let mut from = 0;
    while let Some(i) = memchr::memchr2(first, first.to_ascii_uppercase(), &h[from..=last]) {
        let at = from + i;
        if h[at + 1..at + n.len()].eq_ignore_ascii_case(rest) {
            return true;
        }
        from = at + 1;
    }
    false
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
            ("feature/ABC-12", "feature/abc-12"),
            ("feature/ABC-12", "feature/abc-123"),
            ("aaab", "aab"),
            ("AaAb", "aab"),
            ("ab", "b"),
            ("ab", "abc"),
            ("12 [x]", "[x]"),
            ("Café — the RÉSUMÉ fix", "the r"),
            ("Café — the RÉSUMÉ fix", "fix"),
            ("Café — the RÉSUMÉ fix", "caf"),
            ("Café — the RÉSUMÉ fix", "é — t"),
            ("50 \u{212A} hot", "50 k hot"),
            ("\u{130}STANBUL", "istanbul"),
            ("\u{130}STANBUL", "i"),
            ("\u{130}STANBUL", "stanbul"),
            ("naïve", "na"),
            ("naïve", "nave"),
        ];
        for (hay, needle) in cases {
            assert_eq!(contains_lower(hay, needle), hay.to_lowercase().contains(needle), "{hay:?} / {needle:?}");
        }
    }

    #[test]
    fn contains_lower_agrees_with_lowercasing_on_many_strings() {
        // Every needle cut from every haystack, and their near misses, over a small alphabet
        // with repeats (where a naive first-byte search goes wrong).
        let hays = ["aAbAab", "AAAA", "xYzXyZ x", "ab-AB_ab", "[Fix] fIX fix", "a", "-ab-"];
        for hay in hays {
            let low = hay.to_lowercase();
            for i in 0..low.len() {
                for j in i..=low.len() {
                    for needle in [low[i..j].to_owned(), format!("{}a", &low[i..j]), format!("b{}", &low[i..j])] {
                        assert_eq!(contains_lower(hay, &needle), low.contains(&needle), "{hay:?} / {needle:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn only_the_listed_characters_lowercase_to_ascii() {
        let found: Vec<char> = (0x80..=0x10FFFF).filter_map(char::from_u32).filter(|c| c.to_lowercase().any(|l| l.is_ascii())).collect();
        assert_eq!(found, LOWER_TO_ASCII);
    }

    #[test]
    fn an_ascii_needle_agrees_with_lowercasing_in_mixed_text() {
        // Every ASCII run of each lowercased haystack, cut every way, and near misses.
        let hays = ["Ünïcode ünd ASCII", "aé-AÉ_aé", "— x —", "Zoë ZOË zoe", "\u{212A}elvin", "ǅ ǆ Ǆ ab"];
        for hay in hays {
            let low = hay.to_lowercase();
            for run in low.split(|c: char| !c.is_ascii()) {
                for i in 0..run.len() {
                    for j in i + 1..=run.len() {
                        for needle in [run[i..j].to_owned(), format!("{}q", &run[i..j]), format!("q{}", &run[i..j])] {
                            assert_eq!(contains_lower(hay, &needle), low.contains(&needle), "{hay:?} / {needle:?}");
                        }
                    }
                }
            }
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
