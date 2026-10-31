//! `App` and `app`: two repositories on a volume that tells case apart (ext4, case-sensitive
//! APFS, a Windows folder with case sensitivity on), one repository under two spellings on one
//! that doesn't (NTFS, APFS and FAT by default). Glowfly asks the disk, never the OS, so tabs,
//! the recent list and trust follow the volume either way.

use super::tests::{drive, git, state};
use crate::msg::{Msg, RepoMsg, TabsMsg};
use crate::state::trust::{record, recorded};
use glowfly_core::settings::RepoTrust;
use std::path::{Path, PathBuf};

/// A repository with one commit at `dir` (made if missing).
fn repo_at(dir: &Path, file: &str) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join(file), "x\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "first"]);
}

/// Opens `App` and then `app` under `parent` in two tabs. Returns whether the volume kept them
/// apart.
fn open_case_twins(parent: &Path) -> bool {
    let (upper, lower) = (parent.join("App"), parent.join("app"));
    repo_at(&upper, "upper.txt");
    let apart = match std::fs::create_dir(&lower) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(e) => panic!("could not make {}: {e}", lower.display()),
    };
    if apart {
        repo_at(&lower, "lower.txt");
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(upper.clone())));
    let upper_real = st.snap.as_ref().unwrap().real_workdir.clone();
    record(&mut st.settings, &upper_real, RepoTrust::Trusted);
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    drive(&mut st, Msg::Repo(RepoMsg::Open(lower.clone())));
    let snap = st.snap.as_ref().expect("opened");
    if apart {
        assert_eq!(st.tabs.len(), 2, "two repositories, two tabs");
        assert_ne!(st.tabs[0].folder, st.tabs[1].folder);
        assert!(snap.real_workdir.ends_with("app"), "the second tab shows app: {}", snap.real_workdir.display());
        assert_ne!(recorded(&st.settings, &snap.real_workdir), Some(RepoTrust::Trusted), "trusting App doesn't trust app");
        assert_eq!(st.recent.len(), 2, "both listed: {:?}", st.recent);
    } else {
        assert_eq!(st.tabs.len(), 1, "one folder, one tab");
        assert_eq!(st.recent.len(), 1, "listed once: {:?}", st.recent);
    }
    apart
}

fn fresh(base: &Path, tag: &str) -> PathBuf {
    let root = base.join(format!("glowfly-case-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn case_twins_on_the_temp_volume() {
    let root = fresh(&glowfly_core::paths::temp_dir(), "temp");
    let apart = open_case_twins(&root);
    if cfg!(target_os = "linux") {
        assert!(apart, "ext4 keeps App and app apart");
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// `GLOWFLY_TEST_CS_DIR` names a case-sensitive volume or folder (CI makes one on macOS and
/// Windows).
#[test]
fn case_twins_on_a_case_sensitive_volume() {
    let Some(base) = std::env::var_os("GLOWFLY_TEST_CS_DIR").map(PathBuf::from) else {
        eprintln!("note: GLOWFLY_TEST_CS_DIR is not set; skipping the case-sensitive volume check");
        return;
    };
    let root = fresh(&base, "cs");
    assert!(open_case_twins(&root), "{} tells case apart", base.display());
    let _ = std::fs::remove_dir_all(&root);
}
