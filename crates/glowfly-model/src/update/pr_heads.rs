//! Maps each pull request's head SHA (or SHA prefix) to a loaded commit, for the graph's PR
//! badges and build status. Runs on every load, load-more page and PR list, so a full id is
//! one lookup and the short ones share a single pass over the loaded commits.

use crate::state::AppState;
use git2::Oid;
use std::collections::HashMap;

/// Fills `ForgeState::pr_heads` from the pull requests' heads and the loaded snapshot.
pub(super) fn resolve_pr_heads(st: &mut AppState) {
    let (Some(fs), Some(snap)) = (st.session.forge.as_mut(), st.snap.as_ref()) else { return };
    let prs = fs.prs.iter().filter_map(|p| Some((p.id, p.head.as_deref()?)));
    fs.pr_heads = heads(prs, &snap.row_of, snap.commits.iter().map(|c| c.oid));
}

/// Each pull request id's head commit among `loaded`. A full 40-digit id is looked up in
/// `loaded` alone (a miss is final: no other commit can match it); a prefix (7 to 39 hex
/// digits) takes the first of `commits` it starts, scanning them only when some prefix asks.
pub(super) fn heads<'a>(
    prs: impl Iterator<Item = (u64, &'a str)>,
    loaded: &HashMap<Oid, usize>,
    commits: impl IntoIterator<Item = Oid>,
) -> HashMap<u64, Oid> {
    let mut found = HashMap::new();
    let mut short: HashMap<Prefix, Vec<u64>> = HashMap::new();
    for (id, head) in prs {
        if head.len() == 40 {
            if let Some(o) = Oid::from_str(head).ok().filter(|o| loaded.contains_key(o)) {
                found.insert(id, o);
            }
        } else if let Some(p) = Prefix::parse(head) {
            short.entry(p).or_default().push(id);
        }
    }
    let mut lengths: Vec<usize> = short.keys().map(|p| p.digits).collect();
    lengths.sort_unstable();
    lengths.dedup();
    if short.is_empty() {
        return found;
    }
    for oid in commits {
        for &n in &lengths {
            if let Some(ids) = short.remove(&Prefix::of(oid.as_bytes(), n)) {
                found.extend(ids.into_iter().map(|id| (id, oid)));
            }
        }
        if short.is_empty() {
            break;
        }
    }
    found
}

/// The first `digits` hex digits of an id, packed two to a byte with the rest zero.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Prefix {
    digits: usize,
    bytes: [u8; 20],
}

impl Prefix {
    /// A short head of 7 to 39 hex digits (either case); anything else matches no commit.
    fn parse(hex: &str) -> Option<Prefix> {
        if !(7..40).contains(&hex.len()) {
            return None;
        }
        let mut bytes = [0u8; 20];
        for (i, c) in hex.chars().enumerate() {
            let v = c.to_digit(16)? as u8;
            bytes[i / 2] |= if i % 2 == 0 { v << 4 } else { v };
        }
        Some(Prefix { digits: hex.len(), bytes })
    }

    /// The first `digits` hex digits of the id `oid` (its raw bytes).
    fn of(oid: &[u8], digits: usize) -> Prefix {
        let mut bytes = [0u8; 20];
        let whole = digits / 2;
        bytes[..whole].copy_from_slice(&oid[..whole]);
        if digits % 2 == 1 {
            bytes[whole] = oid[whole] & 0xf0;
        }
        Prefix { digits, bytes }
    }
}
