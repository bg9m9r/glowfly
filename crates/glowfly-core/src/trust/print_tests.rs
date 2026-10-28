use super::super::tests::hook;
use super::*;
use crate::settings::{RepoTrust, SavedTrust};
use crate::testutil::*;
use crate::trust::{SubmoduleScan, scan_at};
use std::path::PathBuf;

fn setting(key: &str, value: &str) -> ConfigCommand {
    ConfigCommand { key: key.into(), value: value.into() }
}

fn hooks_dir(dir: &std::path::Path) -> PathBuf {
    dir.join(".git").join("hooks")
}

#[test]
fn the_same_scan_gives_the_same_print() {
    let dir = scratch_repo("print-same");
    set_config(&dir, &[("core.sshCommand", "ssh -v")]);
    hook(&hooks_dir(&dir), "pre-commit");
    let (a, b) = (scan_at(&dir).unwrap().print(), scan_at(&dir).unwrap().print());
    assert_eq!(a, b);
    assert_eq!(a.0.len(), 2, "{a:?}");
    assert!(a.covers(&b) && a.covers(&a));
    assert!(TrustScan::default().print().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn anything_new_or_changed_isn_t_covered() {
    let dir = scratch_repo("print-gained");
    set_config(&dir, &[("core.sshCommand", "ssh -v")]);
    let before = scan_at(&dir).unwrap().print();

    set_config(&dir, &[("credential.helper", "store")]);
    assert!(!before.covers(&scan_at(&dir).unwrap().print()), "a new key");
    git(&dir, &["config", "--unset", "credential.helper"]);
    assert!(before.covers(&scan_at(&dir).unwrap().print()), "back as it was");

    set_config(&dir, &[("core.sshCommand", "ssh -i other")]);
    assert!(!before.covers(&scan_at(&dir).unwrap().print()), "a changed value");
    set_config(&dir, &[("core.sshCommand", "ssh -v")]);

    hook(&hooks_dir(&dir), "post-merge");
    assert!(!before.covers(&scan_at(&dir).unwrap().print()), "a new hook");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn removing_a_setting_or_hook_stays_covered() {
    let dir = scratch_repo("print-removed");
    set_config(&dir, &[("core.sshCommand", "ssh -v"), ("credential.helper", "store")]);
    hook(&hooks_dir(&dir), "pre-commit");
    hook(&hooks_dir(&dir), "post-merge");
    let before = scan_at(&dir).unwrap().print();
    git(&dir, &["config", "--unset", "credential.helper"]);
    std::fs::remove_file(hooks_dir(&dir).join("post-merge")).unwrap();
    assert!(before.covers(&scan_at(&dir).unwrap().print()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_renamed_only_by_letter_case_stays_covered() {
    let dir = scratch_repo("print-case");
    hook(&hooks_dir(&dir), "post-merge");
    let before = scan_at(&dir).unwrap().print();
    std::fs::rename(hooks_dir(&dir).join("post-merge"), hooks_dir(&dir).join("Post-Merge")).unwrap();
    let after = scan_at(&dir).unwrap();
    assert_eq!(after.hooks.len(), 1, "{after:?}");
    assert!(before.covers(&after.print()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_submodule_s_setting_is_another_item() {
    let top = TrustScan { config: vec![setting("core.sshCommand", "ssh -v")], ..TrustScan::default() };
    let in_sub = TrustScan { submodules: vec![SubmoduleScan { path: "lib".into(), scan: top.clone() }], ..TrustScan::default() };
    assert_ne!(top.print(), in_sub.print());
    assert!(!top.print().covers(&in_sub.print()) && !in_sub.print().covers(&top.print()));
    let elsewhere = TrustScan { submodules: vec![SubmoduleScan { path: "other".into(), scan: top.clone() }], ..TrustScan::default() };
    assert!(!in_sub.print().covers(&elsewhere.print()), "another submodule");
}

#[test]
fn a_checked_out_submodule_s_findings_are_in_the_print() {
    let (parent, sub) = with_submodule("print-sub");
    let before = scan_at(&parent).unwrap().print();
    set_config(&sub, &[("core.sshCommand", "ssh -v")]);
    let now = scan_at(&parent).unwrap();
    assert_eq!(now.submodules.len(), 1, "{now:?}");
    assert!(!before.covers(&now.print()));
    let gained = now.gained_since(&before);
    assert_eq!(gained.submodules.len(), 1);
    assert_eq!(gained.submodules[0].scan.config, [setting("core.sshCommand", "ssh -v")]);
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(crate::paths::temp_dir().join(format!("glowfly-test-print-sub-lib-{}", std::process::id())));
}

#[test]
fn gained_since_lists_only_what_is_new() {
    let dir = scratch_repo("print-gained-since");
    set_config(&dir, &[("core.sshCommand", "ssh -v")]);
    hook(&hooks_dir(&dir), "pre-commit");
    let before = scan_at(&dir).unwrap().print();
    set_config(&dir, &[("credential.helper", "store")]);
    hook(&hooks_dir(&dir), "post-merge");
    let now = scan_at(&dir).unwrap();
    let gained = now.gained_since(&before);
    assert_eq!(gained.config, [setting("credential.helper", "store")]);
    assert_eq!(gained.hooks, ["post-merge"]);
    assert_eq!(gained.hooks_dir, now.hooks_dir, "the hooks' folder stays with them");
    assert!(gained.submodules.is_empty());
    assert!(now.gained_since(&now.print()).is_empty());
    // No new hooks: no folder either.
    let only_setting = TrustScan { hooks: Vec::new(), ..now.clone() }.gained_since(&before);
    assert_eq!(only_setting.hooks_dir, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_stored_hash_doesn_t_drift() {
    // Saved settings hold these: changing the format would ask about every trusted repository.
    // (Hooks held only their name before their scripts were part of it: 415e1133caf14f50 for
    // this one, which no longer matches, so such a repository asks once more.)
    let scan = TrustScan {
        config: vec![setting("core.sshCommand", "ssh -v")],
        hooks: vec!["Post-Merge".into()],
        hook_digests: [("Post-Merge".to_owned(), "0123456789abcdef".to_owned())].into(),
        commands: Some("fedcba9876543210".into()),
        ..TrustScan::default()
    };
    let print = scan.print();
    assert_eq!(print.0.iter().map(String::as_str).collect::<Vec<_>>(), ["5c1d7309520ff92a", "720e8f01e12f413a", "cef9ee38d57dfd9b"]);
}

#[test]
fn allows_follows_the_decision_and_its_print() {
    let clean = TrustScan::default();
    let runs = TrustScan { config: vec![setting("core.sshCommand", "ssh -v")], ..TrustScan::default() };
    assert!(allows(None, &clean));
    assert!(!allows(None, &runs));
    // A decision without a print (saved before prints were kept) covers only what runs nothing.
    assert!(!allows(Some(&SavedTrust::trusted(None)), &runs), "a decision saved before prints asks again");
    assert!(allows(Some(&SavedTrust::trusted(None)), &clean));
    assert!(allows(Some(&SavedTrust::trusted(Some(runs.print()))), &runs));
    assert!(allows(Some(&SavedTrust::trusted(Some(runs.print()))), &clean));
    let stale = SavedTrust::trusted(Some(clean.print()));
    assert!(!allows(Some(&stale), &runs), "made before the setting appeared");
    let refused = SavedTrust { trust: RepoTrust::NotTrusted, print: None };
    assert!(!allows(Some(&refused), &runs) && !allows(Some(&refused), &clean));
}
