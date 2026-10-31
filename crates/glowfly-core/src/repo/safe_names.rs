//! Which names a repository supplies (remotes, upstreams, branches, tags) a load keeps. They end
//! up on git command lines, so a name git could read as an option is left out of the snapshot,
//! as is a remote name git itself would refuse.

/// Whether the configured remote `name` is kept.
pub(crate) fn remote_ok(name: &str) -> bool {
    crate::remote_ops::is_safe_remote_name(name)
}

/// Whether an upstream's `branch.<b>.remote` is kept: "." (a local upstream) or a kept remote
/// name. A URL there is left out too, so the branch shows as tracking nothing.
pub(crate) fn upstream_remote_ok(name: &str) -> bool {
    name == "." || remote_ok(name)
}

/// Whether the ref short name `short` (after `refs/heads/`, `refs/remotes/` or `refs/tags/`)
/// is kept: no part of it may start with '-', so neither the name nor the branch part after a
/// remote's name reads as an option.
pub fn is_safe_ref_name(short: &str) -> bool {
    !short.is_empty() && !short.split('/').any(|p| p.starts_with('-')) && !short.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_read_as_options_are_left_out() {
        assert!(remote_ok("origin") && remote_ok("fork.dev") && remote_ok("a/b"));
        assert!(!remote_ok("-x") && !remote_ok("--upload-pack=x") && !remote_ok("") && !remote_ok("a b") && !remote_ok("a\nb"));
        assert!(upstream_remote_ok(".") && upstream_remote_ok("origin"));
        assert!(!upstream_remote_ok("-x") && !upstream_remote_ok("https://example.com/r.git"));
        assert!(is_safe_ref_name("main") && is_safe_ref_name("feature/a-b") && is_safe_ref_name("origin/x-") && is_safe_ref_name("v1.0"));
        assert!(
            !is_safe_ref_name("-x")
                && !is_safe_ref_name("origin/-x")
                && !is_safe_ref_name("feature/--force")
                && !is_safe_ref_name("")
                && !is_safe_ref_name("a\u{7}")
        );
    }
}
