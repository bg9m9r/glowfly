//! The crates depend on each other in one direction only (docs/ARCHITECTURE.md, Layout), read
//! from `cargo tree` for this machine's target: the model and the runner never pull in egui or
//! the window stack, the views never the runner (not even for their tests), the flow tests never
//! egui, and the app's own build never carries a test-only feature or egui_kittest. Each check
//! first asserts that the tree holds a crate it must hold, so output the parser can't read fails
//! instead of passing. Skipped, with a note, when cargo can't be started.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

/// The window and drawing stack the model and the runner must stay clear of.
const GUI_CRATES: &[&str] = &["egui", "eframe", "egui-wgpu", "egui-winit", "egui_kittest", "winit", "wgpu"];

/// The workspace's root folder.
fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `cargo tree -p <package> -e <edges>` as (package name, its features) per line, or None
/// when cargo can't be started.
fn tree(package: &str, edges: &str) -> Option<Vec<(String, String)>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(workspace())
        .args(["tree", "--quiet", "-p", package, "-e", edges, "--prefix", "none", "--no-dedupe", "-f", "{p}|{f}"])
        .output();
    let out = match out {
        Ok(out) => out,
        Err(e) => {
            eprintln!("skipped: cargo can't be started ({e})");
            return None;
        }
    };
    assert!(out.status.success(), "cargo tree -p {package} failed:\n{}", String::from_utf8_lossy(&out.stderr));
    Some(parse(&String::from_utf8_lossy(&out.stdout)))
}

/// The (name, features) pairs of `cargo tree --prefix none -f "{p}|{f}"` output.
fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (p, f) = l.split_once('|')?;
            Some((p.split_whitespace().next()?.to_owned(), f.trim().to_owned()))
        })
        .collect()
}

/// Asserts that `tree` names every crate in `expected`: a tree that came back empty or in a
/// format `parse` doesn't read would otherwise pass every check.
fn holds(package: &str, tree: &[(String, String)], expected: &[&str]) {
    let names = found(tree, expected);
    let missing: Vec<_> = expected.iter().filter(|e| !names.contains(**e)).collect();
    assert!(
        missing.is_empty(),
        "cargo tree -p {package} doesn't name {missing:?}; has its output format changed?
{tree:?}"
    );
}

/// The names in `tree` that are in `banned`.
fn found(tree: &[(String, String)], banned: &[&str]) -> BTreeSet<String> {
    tree.iter().filter(|(name, _)| banned.contains(&name.as_str())).map(|(name, _)| name.clone()).collect()
}

#[test]
fn the_model_and_the_runner_have_no_gui_crates() {
    for package in ["glowfly-model", "glowfly-runner"] {
        let Some(deps) = tree(package, "normal,build,dev") else { return };
        holds(package, &deps, &[package, "glowfly-core"]);
        let gui = found(&deps, GUI_CRATES);
        assert!(gui.is_empty(), "{package} depends on {gui:?}; keep egui and the window stack in glowfly-ui and the app");
    }
}

#[test]
fn the_views_never_depend_on_the_runner() {
    let Some(deps) = tree("glowfly-ui", "normal,build,dev") else { return };
    holds("glowfly-ui", &deps, &["glowfly-model", "egui", "egui_kittest"]);
    assert!(
        found(&deps, &["glowfly-runner"]).is_empty(),
        "glowfly-ui depends on glowfly-runner; views push messages, and a snapshot whose state needs the runner goes in the app's runner_snapshots/"
    );
}

#[test]
fn the_flow_tests_have_no_egui() {
    let Some(deps) = tree("glowfly-flows", "normal,build,dev") else { return };
    holds("glowfly-flows", &deps, &["glowfly-model", "glowfly-runner"]);
    let gui = found(&deps, &["egui", "eframe", "egui-wgpu", "wgpu"]);
    assert!(gui.is_empty(), "glowfly-flows depends on {gui:?}; flow tests drive update and the runner, never the views");
}

#[test]
fn the_app_build_carries_no_test_support() {
    let Some(deps) = tree("glowfly", "normal,build") else { return };
    holds("glowfly", &deps, &["glowfly-ui", "glowfly-runner", "glowfly-model", "egui"]);
    let leaks: BTreeSet<String> = deps
        .iter()
        .filter(|(name, features)| name.starts_with("glowfly") && features.split(',').any(|f| TEST_FEATURES.contains(&f)))
        .map(|(name, features)| format!("{name} ({features})"))
        .collect();
    assert!(leaks.is_empty(), "the app's build turns on a test-only feature in {leaks:?}; only dev-dependencies may ask for one");
    let kittest = found(&deps, &["egui_kittest"]);
    assert!(kittest.is_empty(), "the app's build pulls in egui_kittest; only glowfly-ui's test-support and dev-dependencies may");
}

/// The workspace crates' features that exist only for tests: the model's, runner's and views'
/// `test-support`, and glowfly-core's fixture `testkit`.
const TEST_FEATURES: &[&str] = &["test-support", "testkit"];

#[test]
fn the_tree_parser_reads_names_and_features() {
    let parsed = parse("glowfly-ui v0.1.0 (C:\\x)|test-support\negui v0.31.1|default,log\nnoise\n");
    assert_eq!(parsed, [("glowfly-ui".into(), "test-support".into()), ("egui".into(), "default,log".into())]);
    assert_eq!(found(&parsed, GUI_CRATES), BTreeSet::from(["egui".to_owned()]));
    holds("glowfly-ui", &parsed, &["glowfly-ui", "egui"]);
}

#[test]
#[should_panic(expected = "doesn't name")]
fn a_tree_without_the_expected_crates_fails() {
    holds(
        "glowfly",
        &parse(
            "not a tree line
",
        ),
        &["egui"],
    );
}
