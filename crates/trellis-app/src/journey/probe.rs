//! A headless dry run of every name and state field the real-input smoke test looks up in the
//! probe file (`scripts/smoke/pass.sh`, `ui/probe.rs`), on the fixture shape that script
//! makes. It runs on every CI OS, so a renamed label or a probe field that stops reporting
//! fails here before the Linux smoke job.

use super::find::{Pick, Region};
use super::fixture::Fixture;
use super::fixture::{self as disk, commit_files, git, write};
use super::harness::Journey;
use serde_json::Value;
use trellis_core::i18n::Lang;

/// The widgets the probe lists in `region` whose name (`label`, else `value`, as the script's
/// `.label // .value`) passes `matches`.
fn named(p: &Value, region: &str, matches: impl Fn(&str) -> bool) -> Vec<Value> {
    let name = |w: &Value| w["label"].as_str().or(w["value"].as_str()).map(str::to_owned);
    let widgets = p["widgets"].as_array().expect("a widget list");
    widgets.iter().filter(|w| (region == "any" || w["region"] == region) && name(w).is_some_and(|n| matches(&n))).cloned().collect()
}

#[track_caller]
fn one(p: &Value, region: &str, label: &str) -> Value {
    let found = named(p, region, |n| n == label);
    assert_eq!(found.len(), 1, "{label:?} in {region}: {found:#?}\nall: {:#?}", named(p, region, |_| true));
    found[0].clone()
}

#[test]
fn the_probe_names_what_the_smoke_test_looks_for() {
    let fx = Fixture::new("probe");
    let repo = fx.work().join("smoke");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    disk::configure(&repo);
    commit_files(&repo, &[("a.txt", "a\n")], "Add a.txt");
    commit_files(&repo, &[("b.txt", "b\n")], "Add b.txt");
    let notes: String = (1..=20).map(|n| format!("line {n}\n")).collect();
    commit_files(&repo, &[("notes.txt", notes.as_str())], "Add notes");
    write(&repo, "todo.txt", "buy milk\n");
    git(&repo, &["add", "todo.txt"]);
    write(&repo, "notes.txt", &format!("{notes}one more line\n"));
    let config = fx.config();
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);

    // After the first load: the rows, HEAD, both drawers, no dialog.
    let p = j.probe();
    one(&p, "central", "Add notes");
    one(&p, "central", "Add b.txt");
    one(&p, "central", "Uncommitted changes");
    assert_eq!(p["head"], disk::head_oid(&repo).to_string());
    assert_eq!(p["head_branch"], "main");
    assert_eq!(p["busy"], false);
    assert_eq!(p["modal"], false, "a startup dialog is in the way");
    assert!(p["panels"]["refs"].is_array() && p["panels"]["detail"].is_array(), "{}", p["panels"]);
    assert_eq!(p["zoom_percent"], 100);
    assert!(p["seq"].is_null(), "the writer adds seq, not the snapshot");

    // The commit box: the summary field, the typed text, the commit button.
    j.select_wip();
    let p = j.probe();
    assert_eq!(p["selection"], "wip");
    assert_eq!(one(&p, "details", "Commit summary")["role"], "TextInput");
    let text = "Smoke: typed with real keys (scale 1)";
    j.type_into("Commit summary", text);
    let p = j.probe();
    assert_eq!(p["draft_summary"], text);
    let button = Lang::En.trn("commit.button.commit", 1, &[("push", &""), ("skip", &"")]);
    assert!(button.starts_with("Commit 1 file"), "{button}");
    let buttons = named(&p, "details", |n| n.starts_with("Commit 1 file"));
    assert_eq!(buttons.len(), 1, "{buttons:#?}");

    // The commit's menu.
    j.right_click("Add notes", Region::Central, Pick::Only);
    let p = j.probe();
    one(&p, "any", "Create branch here…");
    assert!(p["context_menu"] == true || p["popup"] == true, "a menu is open");

    // Its new-branch dialog focuses the name field.
    j.click("Create branch here…", Region::Anywhere, Pick::Only);
    let p = j.probe();
    assert_eq!(p["modal"], true);
    assert_eq!(p["keyboard_focus"]["role"], "TextInput", "the branch name field has the focus");
    j.click("Cancel", Region::Anywhere, Pick::Only);

    // Hiding the left panel takes its rect out of the probe (egui keeps the old one).
    j.run_command("left panel", "Hide left panel");
    let p = j.probe();
    assert!(p["panels"]["refs"].is_null(), "{}", p["panels"]);
    one(&p, "central", "Add b.txt");
    // No journey writes a probe file: the test app has no probe.
    assert!(!config.join("probe.json").exists());
}
