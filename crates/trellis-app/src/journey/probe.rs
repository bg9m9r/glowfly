//! A headless dry run of every name and state field the real-input smoke tests look up in the
//! probe file (`scripts/smoke/pass.sh` for Linux, `scripts/smoke/win-checks.ps1` for Windows,
//! `ui/probe.rs`), on the fixture shape those scripts make. It runs on every CI OS, so a renamed
//! label or a probe field that stops reporting fails here before a smoke job.

use super::find::{Pick, Region};
use super::fixture::Fixture;
use super::fixture::{self as disk, commit_files, git, write};
use super::harness::Journey;
use crate::state::window_chrome::Chrome;
use serde_json::Value;
use std::path::{Path, PathBuf};
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

/// The smoke scripts' fixture: three commits, a staged new file and an unstaged edit.
fn smoke_fixture(fx: &Fixture) -> PathBuf {
    let repo = fx.work().join("smoke");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    disk::configure(&repo);
    commit_files(
        &repo,
        &[(
            "a.txt", "a
",
        )],
        "Add a.txt",
    );
    commit_files(
        &repo,
        &[(
            "b.txt", "b
",
        )],
        "Add b.txt",
    );
    let notes: String = (1..=20)
        .map(|n| {
            format!(
                "line {n}
"
            )
        })
        .collect();
    commit_files(&repo, &[("notes.txt", notes.as_str())], "Add notes");
    write(
        &repo,
        "todo.txt",
        "buy milk
",
    );
    git(&repo, &["add", "todo.txt"]);
    write(
        &repo,
        "notes.txt",
        &format!(
            "{notes}one more line
"
        ),
    );
    repo
}

#[test]
fn the_probe_names_what_the_linux_smoke_test_looks_for() {
    let fx = Fixture::new("probe");
    let repo = smoke_fixture(&fx);
    let config = fx.config();
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);
    let p = j.probe();
    assert_eq!(p["chrome"], "system");
    assert_eq!(p["zoom_percent"], 100);
    assert!(p["panels"]["title-bar"].is_null(), "{}", p["panels"]);
    dry_run_checks(&mut j, &repo, "1");
    // No journey writes a probe file: the test app has no probe.
    assert!(!config.join("probe.json").exists());
}

#[test]
fn the_probe_names_what_the_windows_smoke_test_looks_for() {
    let fx = Fixture::new("probe-windows");
    let repo = smoke_fixture(&fx);
    let mut j = Journey::start_with(fx, Some(repo.clone()), vec![repo.clone()], Chrome::Windows, |s| s.zoom_percent = 150);
    let p = j.probe();
    assert_eq!(p["chrome"], "windows");
    let panels = &p["panels"];
    assert!(panels["title-bar"].is_array() && panels["refs"].is_array() && panels["detail"].is_array(), "{panels}");
    assert!(panels["tabs"].is_null(), "the tabs live in the title bar: {panels}");
    assert_eq!(p["zoom_percent"], 150);
    let (ppp, native) = (p["pixels_per_point"].as_f64().unwrap(), p["native_pixels_per_point"].as_f64().unwrap());
    assert!((ppp - 1.5 * native).abs() <= 0.01, "pixels_per_point {ppp}, native {native}");
    for key in ["title_bar.minimize", "title_bar.maximize", "title_bar.close"] {
        one(&p, "title_bar", Lang::En.tr(key));
    }
    dry_run_checks(&mut j, &repo, "1.5");
}

/// Every lookup of a smoke pass, in its order: the rows, the commit box, the commit's menu and
/// its dialog, and the hidden left panel.
fn dry_run_checks(j: &mut Journey, repo: &Path, scale: &str) {
    // After the first load: the rows, HEAD, both drawers, no dialog.
    let p = j.probe();
    one(&p, "central", "Add notes");
    one(&p, "central", "Add b.txt");
    one(&p, "central", "Add a.txt");
    one(&p, "central", "Uncommitted changes");
    assert_eq!(p["head"], disk::head_oid(repo).to_string());
    assert_eq!(p["head_branch"], "main");
    assert_eq!(p["busy"], false);
    assert_eq!(p["modal"], false, "a startup dialog is in the way");
    assert!(p["panels"]["refs"].is_array() && p["panels"]["detail"].is_array(), "{}", p["panels"]);
    assert!(p["seq"].is_null(), "the writer adds seq, not the snapshot");

    // The commit box: the summary field, the typed text, the commit button.
    j.select_wip();
    let p = j.probe();
    assert_eq!(p["selection"], "wip");
    assert_eq!(one(&p, "details", "Commit summary")["role"], "TextInput");
    let text = format!("Smoke: typed with real keys (scale {scale})");
    j.type_into("Commit summary", &text);
    let p = j.probe();
    assert_eq!(p["draft_summary"], text.as_str());
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
}
