//! The Edit submodule dialog's form and rules: its URL and branch in .gitmodules, prefilled with
//! what is there, and the `git submodule set-url` / `set-branch` commands a change runs.

use glowfly_core::clone_ops::BRANCH_INVALID;
use glowfly_core::repo::SubmoduleInfo;
use glowfly_core::submodule_ops::{add_url_message, check_add_url, check_edit_branch, set_branch_cmd, set_url_cmds};
use std::path::PathBuf;

/// Logged when Save is pressed after the dialog's tab stopped being the active one.
pub const TAB_GONE: &str = "The submodule was not changed: its repository's tab is no longer active.";

/// Logged when Save is pressed with nothing changed.
pub const UNCHANGED: &str = "Nothing to save: the URL and branch are as they were.";

/// The Edit submodule dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct EditForm {
    /// The workdir of the tab the dialog was opened in.
    pub repo: PathBuf,
    /// Repo-relative, with '/'.
    pub path: String,
    /// The .gitmodules name.
    pub name: String,
    pub url: String,
    pub branch: String,
    pub original_url: String,
    pub original_branch: String,
    /// The fields edited and whether Save was tried, so a problem waits for them.
    pub touched: crate::state::touched::Touched,
}

impl EditForm {
    pub fn new(repo: PathBuf, sm: &SubmoduleInfo) -> Self {
        let url = sm.url.clone().unwrap_or_default();
        let branch = sm.branch.clone().unwrap_or_default();
        Self {
            repo,
            path: sm.path.clone(),
            name: sm.name.clone(),
            original_url: url.clone(),
            original_branch: branch.clone(),
            url,
            branch,
            touched: Default::default(),
        }
    }

    pub fn url_problem(&self) -> Option<&'static str> {
        check_add_url(&self.url).err().map(|p| add_url_message(&p))
    }

    pub fn branch_problem(&self) -> Option<&'static str> {
        check_edit_branch(&self.branch).is_err().then_some(BRANCH_INVALID)
    }

    fn url_changed(&self) -> bool {
        self.url.trim() != self.original_url.trim()
    }

    fn branch_changed(&self) -> bool {
        self.branch.trim() != self.original_branch.trim()
    }

    /// Whether Save would change anything (surrounding spaces don't count).
    pub fn changed(&self) -> bool {
        self.url_changed() || self.branch_changed()
    }

    /// The commands Save runs: set-url and sync when the URL changed, then set-branch when the
    /// branch did. Err says why nothing can run.
    pub fn commands(&self) -> Result<Vec<Vec<String>>, String> {
        if let Some(p) = self.url_problem() {
            return Err(p.to_owned());
        }
        let branch = check_edit_branch(&self.branch).map_err(|()| BRANCH_INVALID.to_owned())?;
        if !self.changed() {
            return Err(UNCHANGED.to_owned());
        }
        let mut out = Vec::new();
        if self.url_changed() {
            out.extend(set_url_cmds(&self.path, &self.url));
        }
        if self.branch_changed() {
            out.push(set_branch_cmd(&self.path, branch.as_deref()));
        }
        Ok(out)
    }
}

/// The job's label, in the toast and the activity log.
pub fn job_label(path: &str) -> String {
    format!("Edit submodule {path}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Oid;
    use glowfly_core::repo::SubmoduleState;

    fn form(url: &str, branch: Option<&str>) -> EditForm {
        let id = Oid::from_str(&"a".repeat(40)).unwrap();
        let sm = SubmoduleInfo {
            name: "lib".into(),
            path: "libs/lib".into(),
            url: Some(url.into()),
            clone_url: None,
            branch: branch.map(str::to_owned),
            added: false,
            recorded: id,
            checked_out: Some(id),
            state: SubmoduleState::InSync,
            left_behind: Vec::new(),
            outside_sparse: false,
        };
        EditForm::new(PathBuf::from("repo"), &sm)
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn prefilled_and_unchanged_until_edited() {
        let mut f = form("../lib.git", Some("dev"));
        assert_eq!((f.url.as_str(), f.branch.as_str(), f.name.as_str()), ("../lib.git", "dev", "lib"));
        assert!(!f.changed());
        f.url = " ../lib.git ".into();
        f.branch = "dev ".into();
        assert!(!f.changed(), "surrounding spaces don't count");
        assert_eq!(f.commands(), Err(UNCHANGED.to_owned()));
    }

    #[test]
    fn commands_in_order() {
        let mut f = form("../lib.git", Some("dev"));
        f.url = "https://h/o/lib.git".into();
        let url =
            [s(&["submodule", "set-url", "--", "libs/lib", "https://h/o/lib.git"]), s(&["submodule", "sync", "--", ":(literal)libs/lib"])];
        assert_eq!(f.commands().unwrap(), url);
        f.branch = String::new();
        let mut all = url.to_vec();
        all.push(s(&["submodule", "set-branch", "--default", "--", "libs/lib"]));
        assert_eq!(f.commands().unwrap(), all);
        f.url = "../lib.git".into();
        f.branch = ".".into();
        assert_eq!(f.branch_problem(), None, "'.' follows the parent's branch");
        assert_eq!(f.commands().unwrap(), [s(&["submodule", "set-branch", "--branch", ".", "--", "libs/lib"])]);
    }

    #[test]
    fn problems() {
        let mut f = form("../lib.git", None);
        f.url = "-x".into();
        assert_eq!(f.url_problem(), Some("URLs can't start with a dash."));
        assert!(f.commands().is_err());
        f.url = "../lib.git".into();
        f.branch = "bad..name".into();
        assert_eq!(f.branch_problem(), Some(BRANCH_INVALID));
        assert_eq!(f.commands(), Err(BRANCH_INVALID.to_owned()));
        assert_eq!(job_label("libs/lib"), "Edit submodule libs/lib");
    }
}
