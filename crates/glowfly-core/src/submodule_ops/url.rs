//! Relative submodule URLs (`./x`, `../x` in .gitmodules): git resolves them against the
//! parent's default remote URL, or its folder when it has none. Pure string work: the base is a
//! URL, an scp-like `host:path` or a '/'-separated path, never an OS path.

/// Whether `url` is relative to the parent's remote (`./x` or `../x`).
pub fn is_relative_url(url: &str) -> bool {
    let u = url.trim();
    u.starts_with("./") || u.starts_with("../")
}

/// `url` resolved against `base` the way git does it: each leading `../` drops one part of the
/// base (never the `scheme://host` of a URL; the `:` of `host:path` separates like a `/`), each
/// `./` is skipped, and the rest is joined on. A URL that isn't relative comes back trimmed.
pub fn resolve_url(url: &str, base: &str) -> String {
    let mut rel = url.trim();
    if !is_relative_url(rel) {
        return rel.to_owned();
    }
    let mut base = base.trim().to_owned();
    let floor = floor(&base);
    trim_end(&mut base, floor);
    loop {
        if let Some(r) = rel.strip_prefix("../") {
            drop_last(&mut base, floor);
            rel = r;
        } else if let Some(r) = rel.strip_prefix("./") {
            rel = r;
        } else {
            break;
        }
    }
    if base.is_empty() || base.ends_with(':') { base + rel } else { format!("{base}/{rel}") }
}

/// Where the part of `base` that `../` may drop begins: after a URL's `scheme://host`, after the
/// `:` of an scp-like `host:path`, else at the start.
fn floor(base: &str) -> usize {
    if let Some(i) = base.find("://") {
        let host = i + 3;
        return base[host..].find('/').map_or(base.len(), |j| host + j);
    }
    match (base.find(':'), base.find('/')) {
        (Some(c), Some(s)) if c < s => c + 1,
        (Some(c), None) => c + 1,
        _ => 0,
    }
}

/// Drops trailing `/` and `/.` (above `floor`).
fn trim_end(base: &mut String, floor: usize) {
    loop {
        if base.len() > floor.max(1) && base.ends_with('/') {
            base.pop();
        } else if base.len() >= floor + 2 && base.ends_with("/.") {
            base.truncate(base.len() - 2);
        } else {
            return;
        }
    }
}

/// Drops the last part of `base`: back to its last `/` above `floor`, else to `floor` itself
/// (the `:` of `host:path`, or nothing for a single relative part).
fn drop_last(base: &mut String, floor: usize) {
    match base[floor..].rfind('/') {
        Some(i) => base.truncate(floor + i),
        None if floor > 0 && base[..floor].ends_with(':') => base.truncate(floor),
        None if floor == 0 => base.clear(),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_urls_resolve_like_git() {
        let cases = [
            ("../lib.git", "https://h/o/parent.git", "https://h/o/lib.git"),
            ("./lib.git", "https://h/o/parent.git", "https://h/o/parent.git/lib.git"),
            ("../lib", "https://h/o/parent/", "https://h/o/lib"),
            ("../lib.git", "git@h:o/parent.git", "git@h:o/lib.git"),
            ("../lib.git", "git@h:parent.git", "git@h:lib.git"),
            ("../../x", "/srv/git/parent", "/srv/x"),
            ("../lib", "C:/r/parent", "C:/r/lib"),
            ("../../../x", "https://h/o/p", "https://h/x"),
            ("./../lib", "https://h/o/p/.", "https://h/o/lib"),
            (" https://other/lib.git ", "https://h/o/p", "https://other/lib.git"),
            ("/abs/lib", "https://h/o/p", "/abs/lib"),
        ];
        for (url, base, want) in cases {
            assert_eq!(resolve_url(url, base), want, "{url} against {base}");
        }
    }

    #[test]
    fn which_urls_are_relative() {
        assert!(is_relative_url("./x") && is_relative_url(" ../x"));
        assert!(!is_relative_url(".lib") && !is_relative_url("https://h/x") && !is_relative_url("lib"));
    }
}
