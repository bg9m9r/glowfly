//! A remote's refspecs as `push_target` reads them: its `remote.<r>.push` specs, which decide a
//! plain push before `push.default` does, its fetch specs, which name the remote-tracking ref a
//! pushed ref shows up as, and `remote.<r>.mirror`. Declared from `push_target.rs`.

use git2::{Direction, Repository};

/// One refspec's source and destination (`+` dropped); a pattern holds one `*`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Spec {
    src: String,
    dst: Option<String>,
}

/// What `push_target` needs of one remote's config.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct RemoteSpecs {
    /// `remote.<r>.push`, negative (`^`) specs left out.
    pub push: Vec<Spec>,
    /// `remote.<r>.fetch`, negative specs left out.
    pub fetch: Vec<Spec>,
    /// `remote.<r>.mirror`.
    pub mirror: bool,
}

/// The refspecs and mirror setting of the configured remote `name`; empty when it can't be read.
pub(super) fn read(repo: &Repository, name: &str) -> RemoteSpecs {
    let mut out = RemoteSpecs::default();
    if let Ok(remote) = repo.find_remote(name) {
        for spec in remote.refspecs() {
            if spec.str().is_some_and(|s| s.starts_with('^')) {
                continue;
            }
            let Some(src) = spec.src().filter(|s| !s.is_empty()) else { continue };
            let s = Spec { src: src.to_owned(), dst: spec.dst().filter(|d| !d.is_empty()).map(str::to_owned) };
            match spec.direction() {
                Direction::Push => out.push.push(s),
                Direction::Fetch => out.fetch.push(s),
            }
        }
    }
    out.mirror = repo.config().and_then(|c| c.get_bool(&format!("remote.{name}.mirror"))).unwrap_or(false);
    out
}

impl Spec {
    #[cfg(test)]
    pub(super) fn new(src: &str, dst: Option<&str>) -> Spec {
        Spec { src: src.to_owned(), dst: dst.map(str::to_owned) }
    }

    /// `name` mapped through this spec: its destination (the same ref without one), or None
    /// when the source doesn't match. A pattern matches as git's does, the part `*` stands for
    /// put in the destination's `*`.
    fn map(&self, name: &str) -> Option<String> {
        if let Some((prefix, suffix)) = self.src.split_once('*') {
            let rest = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
            return Some(match &self.dst {
                Some(d) => d.replacen('*', rest, 1),
                None => name.to_owned(),
            });
        }
        (full_name(&self.src) == name).then(|| self.dst.as_deref().map(full_name).unwrap_or_else(|| name.to_owned()))
    }
}

/// A short branch name as a full ref (`main` is refs/heads/main); a full one as it is.
fn full_name(name: &str) -> String {
    if name.starts_with("refs/") { name.to_owned() } else { format!("refs/heads/{name}") }
}

/// Where the push specs send the branch ref `full`: the first spec whose source matches. A
/// source of `HEAD` (or `@`) is the checked-out branch, so it matches only when `is_head`; it
/// keeps the branch's name without a destination, or with `HEAD`/`@` as one (libgit2 fills a
/// push spec's missing destination with its source, so a bare `HEAD` reads as `HEAD:HEAD`).
/// None: a plain push doesn't send it.
pub(super) fn push_dest(specs: &[Spec], full: &str, is_head: bool) -> Option<String> {
    let is_head_name = |n: &str| n == "HEAD" || n == "@";
    specs.iter().find_map(|s| {
        if is_head_name(&s.src) {
            return is_head.then(|| match s.dst.as_deref() {
                Some(d) if !is_head_name(d) => full_name(d),
                _ => full.to_owned(),
            });
        }
        s.map(full)
    })
}

/// The remote-tracking ref the remote's `dest` ref is fetched into (refs/heads/main on fork to
/// refs/remotes/fork/main): the first fetch spec whose source matches. None when none does.
pub(super) fn tracking_ref(fetch: &[Spec], dest: &str) -> Option<String> {
    fetch.iter().filter(|s| s.dst.is_some()).find_map(|s| s.map(dest))
}
