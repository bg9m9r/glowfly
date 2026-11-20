//! Tests for the metadata of the refs-only repository check (effect/meta.rs): a load that
//! only compares the ref fingerprint is named `check-refs`, apart from the full `load-repo`.

use crate::effect::{Effect, KnownRepo, LoadEffect, LoadKind};

fn repo_load(known: Option<KnownRepo>) -> Effect {
    Effect::Load(LoadEffect::Repo {
        path: glowfly_core::paths::temp_dir().join("glowfly-meta-refs"),
        opts: Default::default(),
        generation: 1,
        kind: LoadKind::Refresh,
        known,
        prior: None,
    })
}

#[test]
fn a_refs_only_load_is_named_check_refs_and_a_full_load_load_repo() {
    let refs_only = KnownRepo { refs_key: 7, refs_only: true, submodules: false, open: None, git_status: false };
    let full = KnownRepo { refs_only: false, ..refs_only.clone() };
    for (known, name) in [(None, "load-repo"), (Some(full), "load-repo"), (Some(refs_only), "check-refs")] {
        let effect = repo_load(known);
        assert_eq!(effect.name(), name, "{effect:?}");
        assert_eq!(effect.scope(), None, "{name} has no scope");
        assert_eq!(effect.queue(), None, "{name} has no queue");
        assert!(effect.timed(), "{name} is timed");
    }
}
