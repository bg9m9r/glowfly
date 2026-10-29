//! Reading the last installer run from the update log: each outcome, the runs that changed
//! nothing, and a run reported once.

use super::*;

const STAMP: &str = "2026-10-29 12:00:00 ";

fn log(lines: &[&str]) -> String {
    lines.iter().map(|l| format!("{STAMP}{l}\n")).collect()
}

fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-report-{tag}-{}", std::process::id()));
    let _ = crate::persist::remove_tree(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn each_installer_outcome() {
    let run = |outcome: &str| last_problem(&log(&["Glowfly has quit; installing 0.3.0", "running C:\\msiexec.exe on x.msi", outcome]));
    let v = || "0.3.0".to_owned();
    assert_eq!(run("the installer was cancelled"), Some((v(), InstallProblem::Cancelled)));
    assert_eq!(run("another installation is running; try the update again later"), Some((v(), InstallProblem::Busy)));
    assert_eq!(run("the installer failed (Some(1603)); its log is C:\\t\\install.log"), Some((v(), InstallProblem::Failed(Some(1603)))));
    assert_eq!(run("the installer failed (None); its log is C:\\t\\install.log"), Some((v(), InstallProblem::Failed(None))));
    assert_eq!(run("couldn't run the installer: SystemRoot isn't set"), Some((v(), InstallProblem::NotRun("SystemRoot isn't set".into()))));
    assert_eq!(run("the installer finished"), None);
}

#[test]
fn log_lines_lose_their_time() {
    assert_eq!(text(&format!("{STAMP}{INSTALLING}0.3.0")), "Glowfly has quit; installing 0.3.0");
    assert_eq!(text("no stamp"), "no stamp");
    assert_eq!(text("2026-10-29 12:00 short"), "2026-10-29 12:00 short");
}

#[test]
fn only_the_last_run_counts() {
    let cancelled_then_installed = log(&[
        "Glowfly has quit; installing 0.3.0",
        "the installer was cancelled",
        "started C:\\Glowfly\\glowfly.exe",
        "Glowfly has quit; installing 0.3.1",
        "the installer finished",
        "started C:\\Glowfly\\glowfly.exe",
    ]);
    assert_eq!(last_problem(&cancelled_then_installed), None);
    let then_nothing =
        log(&["Glowfly has quit; installing 0.3.0", "the installer was cancelled", "not updating: Glowfly didn't hand over the update"]);
    assert_eq!(last_problem(&then_nothing), None);
    // A portable swap runs no installer.
    let swap = log(&["Glowfly has quit; installing 0.3.0", "removed C:\\Glowfly\\glowfly.exe.bak", "started C:\\Glowfly\\glowfly.exe"]);
    assert_eq!(last_problem(&swap), None);
    assert_eq!(last_problem(""), None);
}

#[test]
fn a_run_is_reported_once() {
    let dir = scratch("once");
    assert_eq!(take(&dir), None, "no log yet");
    let lines = log(&["Glowfly has quit; installing 0.3.0", "the installer was cancelled", "started /x/glowfly"]);
    std::fs::write(dir.join(LOG_FILE), lines).unwrap();
    let got = take(&dir).expect("reported");
    assert_eq!((got.version.as_str(), &got.problem, &got.log), ("0.3.0", &InstallProblem::Cancelled, &dir.join(LOG_FILE)));
    assert_eq!(take(&dir), None, "only once");
    assert!(std::fs::read_to_string(dir.join(LOG_FILE)).unwrap().trim_end().ends_with(SHOWN));
    let _ = crate::persist::remove_tree(&dir);
}

#[test]
fn a_long_log_is_read_from_its_end() {
    let dir = scratch("long");
    let mut text = log(&["Glowfly has quit; installing 0.2.0", "the installer was cancelled"]);
    text.push_str(&"x".repeat(TAIL_BYTES as usize));
    text.push('\n');
    text.push_str(&log(&["Glowfly has quit; installing 0.3.0", "the installer failed (Some(1603)); its log is /t/install.log"]));
    std::fs::write(dir.join(LOG_FILE), text).unwrap();
    let got = take(&dir).expect("reported");
    assert_eq!((got.version.as_str(), got.problem), ("0.3.0", InstallProblem::Failed(Some(1603))));
    let _ = crate::persist::remove_tree(&dir);
}
