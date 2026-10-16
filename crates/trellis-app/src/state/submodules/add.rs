//! The Add submodule dialog's form and rules: the URL, the folder (checked as typed against the
//! text and the snapshot, then on disk by the runner) and the optional branch.

use std::path::PathBuf;
use trellis_core::clone_ops::{BRANCH_INVALID, check_clone_branch};
use trellis_core::repo::Snapshot;
use trellis_core::submodule_ops::{AddPathProblem, add_url_message, check_add_url, parse_add_path};

/// Logged when Add submodule is pressed after the dialog's tab stopped being the active one.
pub const TAB_GONE: &str = "The submodule was not added: its repository's tab is no longer active.";

/// The Add submodule dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct AddForm {
    /// The workdir of the tab the dialog was opened in.
    pub repo: PathBuf,
    pub url: String,
    /// The folder for the submodule as typed: relative to the repository, or a full path inside it.
    pub path: String,
    /// The user typed in the folder field, so it no longer follows the URL.
    pub path_edited: bool,
    pub branch: String,
    /// The repo path the disk check was last asked for.
    pub asked: Option<String>,
    /// The runner's answer and the repo path it is for: Ok(warnings), or why git would refuse.
    pub checked: Option<(String, Result<Vec<String>, String>)>,
}

/// What the disk check says about the folder as it is typed now.
#[derive(Debug, PartialEq)]
pub enum Disk<'a> {
    /// No answer for this text yet.
    Waiting,
    Ok(&'a [String]),
    Err(&'a str),
}

/// A checked request to add a submodule.
#[derive(Clone, Debug, PartialEq)]
pub struct AddRequest {
    pub url: String,
    /// Repo-relative, with '/'.
    pub path: String,
    pub branch: Option<String>,
}

impl AddForm {
    pub fn new(repo: PathBuf) -> Self {
        Self { repo, url: String::new(), path: String::new(), path_edited: false, branch: String::new(), asked: None, checked: None }
    }

    /// The folder as a repo path, or why it can't take a submodule (from the text and the
    /// snapshot's submodules; the disk is the runner's).
    pub fn rel(&self, snap: &Snapshot) -> Result<String, String> {
        let rel = parse_add_path(&self.path, &self.repo).map_err(|p| p.message(self.path.trim()))?;
        match snapshot_problem(snap, &rel) {
            Some(p) => Err(p.message(&rel)),
            None => Ok(rel),
        }
    }

    pub fn url_problem(&self) -> Option<&'static str> {
        check_add_url(&self.url).err().map(|p| add_url_message(&p))
    }

    pub fn branch_problem(&self) -> Option<&'static str> {
        check_clone_branch(&self.branch).is_err().then_some(BRANCH_INVALID)
    }

    /// The disk check's answer for the repo path `rel`, if it has come back for that path.
    pub fn disk_state(&self, rel: &str) -> Disk<'_> {
        match &self.checked {
            Some((p, Ok(w))) if p == rel => Disk::Ok(w),
            Some((p, Err(e))) if p == rel => Disk::Err(e),
            _ => Disk::Waiting,
        }
    }

    /// The request the form makes now, or the first reason it can't be sent. The disk check is
    /// not part of it (see `disk_state`).
    pub fn request(&self, snap: &Snapshot) -> Result<AddRequest, String> {
        if let Some(p) = self.url_problem() {
            return Err(p.to_owned());
        }
        let path = self.rel(snap)?;
        let branch = check_clone_branch(&self.branch).map_err(|()| BRANCH_INVALID.to_owned())?;
        Ok(AddRequest { url: self.url.trim().to_owned(), path, branch })
    }
}

/// How the repo path `rel` collides with a submodule the snapshot lists.
pub fn snapshot_problem(snap: &Snapshot, rel: &str) -> Option<AddPathProblem> {
    let under = |inner: &str, outer: &str| inner.len() > outer.len() && inner.starts_with(outer) && inner.as_bytes()[outer.len()] == b'/';
    snap.submodules.iter().find_map(|sm| {
        if sm.path == rel {
            Some(AddPathProblem::IsSubmodule(sm.path.clone()))
        } else if under(rel, &sm.path) {
            Some(AddPathProblem::InsideSubmodule(sm.path.clone()))
        } else if under(&sm.path, rel) {
            Some(AddPathProblem::ContainsSubmodule(sm.path.clone()))
        } else {
            None
        }
    })
}

/// The hint under a relative URL (`./x`, `../x`): what git resolves it against.
pub fn relative_url_hint(url: &str) -> Option<&'static str> {
    let u = url.trim();
    let relative = ["./", "../", ".\\", "..\\"].iter().any(|p| u.starts_with(p));
    relative
        .then_some("A relative URL is resolved against this repository's default remote URL, or against its folder when it has no remote.")
}

/// The job's label, in the toast and the activity log.
pub fn job_label(path: &str) -> String {
    format!("Add submodule {path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_and_labels() {
        assert!(relative_url_hint("../lib.git").is_some() && relative_url_hint(" ./lib").is_some());
        assert!(relative_url_hint("https://h/o/lib.git").is_none() && relative_url_hint(".lib").is_none());
        assert_eq!(job_label("libs/lib"), "Add submodule libs/lib");
    }

    #[test]
    fn disk_answers_only_for_the_current_path() {
        let mut f = AddForm::new(PathBuf::from("C:/r"));
        assert_eq!(f.disk_state("lib"), Disk::Waiting);
        f.checked = Some(("lib".into(), Ok(vec!["w".into()])));
        assert_eq!(f.disk_state("lib"), Disk::Ok(&["w".to_owned()]));
        assert_eq!(f.disk_state("lib2"), Disk::Waiting);
        f.checked = Some(("lib".into(), Err("no".into())));
        assert_eq!(f.disk_state("lib"), Disk::Err("no"));
    }

    #[test]
    fn url_and_branch_problems() {
        let mut f = AddForm::new(PathBuf::from("C:/r"));
        assert_eq!(f.url_problem(), Some("Enter the URL or path of the repository to add."));
        f.url = "-x".into();
        assert_eq!(f.url_problem(), Some("URLs can't start with a dash."));
        f.url = "https://h/o/lib.git".into();
        assert_eq!(f.url_problem(), None);
        f.branch = "bad..name".into();
        assert_eq!(f.branch_problem(), Some(BRANCH_INVALID));
        f.branch = " dev ".into();
        assert_eq!(f.branch_problem(), None);
    }
}
