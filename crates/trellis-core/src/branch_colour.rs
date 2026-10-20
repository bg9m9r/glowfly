//! Branch colours: which of the palette's branch hues each owner of the graph's lanes gets.
//!
//! A branch's hue comes from its name (FNV-1a, so it is the same in every session and on every
//! machine), with its remote copies sharing it: `origin/feature` and `feature` are one key. The
//! repository's main branch takes the accent instead (`Swatch::Main`). Branches that ever sit in
//! side-by-side lanes never share a hue: `resolve` moves the later one to the next free slot.
//! Pins (`Colouring::pins`) fix a branch's slot whatever its neighbours.

use crate::graph::owners::{Owner, Owners};
use std::collections::BTreeMap;

/// How many branch hues the palette has (the accent is not one of them).
pub const SLOTS: u8 = 14;

/// The colour a branch is drawn in: the accent for the main branch, else one of the hues.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Swatch {
    Main,
    Slot(u8),
}

/// FNV-1a, 32-bit: stable across runs and platforms, unlike std's randomised hasher.
pub fn name_hash(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |h, &b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

/// The slot (0 to `slots` − 1) a name prefers.
pub fn slot_of(bytes: &[u8], slots: u8) -> u8 {
    (name_hash(bytes) % u32::from(slots.max(1))) as u8
}

/// The colour key of the full ref name `full`: a local branch's name, or a remote-tracking
/// branch's name without its remote. `remotes` are the configured remote names; the longest that
/// matches is stripped (remote names may hold '/'), else the first path segment. None for a
/// remote's HEAD, tags and other refs. Case is kept, as git's ref names are case-sensitive.
pub fn branch_key<'a>(full: &'a str, remotes: &[&str]) -> Option<&'a str> {
    if let Some(name) = full.strip_prefix("refs/heads/") {
        return (!name.is_empty()).then_some(name);
    }
    let rest = full.strip_prefix("refs/remotes/")?;
    if rest.ends_with("/HEAD") {
        return None;
    }
    let remote = remotes
        .iter()
        .filter(|r| !r.is_empty() && rest.len() > r.len() && rest.starts_with(**r) && rest.as_bytes()[r.len()] == b'/')
        .max_by_key(|r| r.len());
    let key = match remote {
        Some(r) => &rest[r.len() + 1..],
        None => rest.split_once('/')?.1,
    };
    (!key.is_empty()).then_some(key)
}

/// How branches are coloured: the number of slots in use and the slots pinned by key.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Colouring {
    /// How many slots to pick from; 0 means `SLOTS`.
    pub slots: u8,
    /// Branch keys with a chosen slot, kept whatever their neighbours.
    pub pins: BTreeMap<String, u8>,
}

impl Colouring {
    /// The number of slots in use.
    pub fn n(&self) -> u8 {
        if self.slots == 0 { SLOTS } else { self.slots }
    }

    /// The slot `key` gets when it isn't in the graph: its pin, else its name's slot.
    pub fn slot_for(&self, key: &str) -> u8 {
        let n = self.n();
        self.pins.get(key).map_or_else(|| slot_of(key.as_bytes(), n), |p| p % n)
    }
}

/// The slot an owner prefers: its name's, or for an unnamed line its commit id's.
pub fn preferred(owner: &Owner, n: u8) -> u8 {
    match owner {
        Owner::Branch(key) => slot_of(key.as_bytes(), n),
        Owner::Unnamed(oid) => slot_of(oid.as_bytes(), n),
    }
}

/// One swatch per owner in `owners.list`: Main for the main branch, the pinned slot for pinned
/// branches, and for the others (in order of first appearance) their preferred slot unless an
/// already-decided neighbour has it, then the next slot in palette order no decided neighbour
/// uses. With every slot taken an owner keeps its preferred one.
pub fn resolve(owners: &Owners, c: &Colouring) -> Vec<Swatch> {
    let n = c.n();
    let len = owners.list.len();
    let mut adjacent: Vec<Vec<u32>> = vec![Vec::new(); len];
    for &(a, b) in &owners.neighbours {
        if (a as usize) < len && (b as usize) < len {
            adjacent[a as usize].push(b);
            adjacent[b as usize].push(a);
        }
    }
    let mut out: Vec<Option<Swatch>> = vec![None; len];
    for (i, owner) in owners.list.iter().enumerate() {
        if let Owner::Branch(key) = owner {
            if owners.mainline.as_deref() == Some(key.as_str()) {
                out[i] = Some(Swatch::Main);
            } else if let Some(&pin) = c.pins.get(key) {
                out[i] = Some(Swatch::Slot(pin % n));
            }
        }
    }
    for (i, owner) in owners.list.iter().enumerate() {
        if out[i].is_some() {
            continue;
        }
        let want = preferred(owner, n);
        let used = |s: u8| adjacent[i].iter().any(|&j| out[j as usize] == Some(Swatch::Slot(s)));
        let pick = (0..n).map(|k| (want + k) % n).find(|&s| !used(s)).unwrap_or(want);
        out[i] = Some(Swatch::Slot(pick));
    }
    out.into_iter().map(|s| s.unwrap_or(Swatch::Main)).collect()
}

#[cfg(test)]
#[path = "branch_colour_tests.rs"]
mod tests;
