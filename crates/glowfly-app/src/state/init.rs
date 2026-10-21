//! The Init dialog's form: the folder (checked on disk by an effect, never by the view), the
//! initial branch, a `.gitignore` template, a license and whether to commit the new files, with
//! the pure rules that check them and the request they turn into.

use glowfly_core::init_ops::{
    FolderFacts, GitignoreTemplate, INITIAL_COMMIT, InitPlan, LICENSE_FILE, License, NewFile, check_holder, gitignore_text, license_text,
};
use std::path::PathBuf;

pub const RELATIVE: &str = "Enter the full path of the folder (it may start with ~).";
pub const NO_PARENT: &str = "Its parent folder doesn't exist.";
pub const NEW_FOLDER: &str = "A new folder is created.";
pub const IS_REPO: &str = "This folder is already a Git repository. Git keeps its history; only the files you choose below are added.";
pub const HAS_FILES: &str = "The files already in this folder are left as they are (not committed).";
pub const GITIGNORE_KEPT: &str = "This folder already has a .gitignore; it is kept.";
pub const BRANCH_INVALID: &str = "Not a valid branch name.";

#[derive(Clone, Debug, PartialEq)]
pub struct InitForm {
    pub folder: String,
    /// The folder text last checked on disk (trimmed) and what was found there. Set by `update`
    /// from `InitMsg::FolderChecked`.
    pub folder_checked: Option<(String, Option<FolderFacts>)>,
    /// Empty is git's default.
    pub branch: String,
    pub gitignore: Option<GitignoreTemplate>,
    /// Add the operating system and editor section to the `.gitignore`.
    pub os_editor: bool,
    pub license: Option<License>,
    /// The copyright holder (MIT and BSD).
    pub holder: String,
    /// The name Trellis filled in (git's user.name); replaced by a later suggestion only while
    /// the field still shows it.
    pub holder_suggested: String,
    /// Commit the files written as "Initial commit".
    pub commit: bool,
    /// The fields edited and whether Create was tried, so a problem waits for them.
    pub touched: super::touched::Touched,
}

/// What blocks Create repository: each field's red line, and whether it is blocked (silent
/// problems included).
#[derive(Debug, Default, PartialEq)]
pub struct InitProblems {
    pub folder: Option<String>,
    pub branch: Option<String>,
    pub holder: Option<String>,
    pub blocked: bool,
}

/// A line under the folder field.
#[derive(Debug, PartialEq)]
pub enum FolderLine {
    Problem(&'static str),
    Note(&'static str),
}

/// An init to run: the folder (resolved) and what to do in it.
#[derive(Clone, Debug, PartialEq)]
pub struct InitRequest {
    pub folder: PathBuf,
    pub plan: InitPlan,
}

impl InitForm {
    pub fn new(folder: String, branch: String) -> Self {
        Self {
            folder,
            folder_checked: None,
            branch,
            gitignore: None,
            os_editor: false,
            license: None,
            holder: String::new(),
            holder_suggested: String::new(),
            commit: true,
            touched: super::touched::Touched::default(),
        }
    }

    /// The last check of the folder text now in the field: None until that text was checked,
    /// Some(None) when it names no usable folder.
    pub fn checked(&self) -> Option<Option<&FolderFacts>> {
        self.folder_checked.as_ref().filter(|(text, _)| text == self.folder.trim()).map(|(_, found)| found.as_ref())
    }

    /// What the check found in the folder now in the field.
    pub fn facts(&self) -> Option<&FolderFacts> {
        self.checked().flatten()
    }

    /// The line under the folder field, if any.
    pub fn folder_line(&self) -> Option<FolderLine> {
        let text = self.folder.trim();
        if text.is_empty() {
            return None;
        }
        if !glowfly_core::paths::typed_is_absolute(text) {
            return Some(FolderLine::Problem(RELATIVE));
        }
        match self.checked()? {
            None => Some(FolderLine::Problem(NO_PARENT)),
            Some(f) if !f.exists => Some(FolderLine::Note(NEW_FOLDER)),
            Some(f) if f.repo => Some(FolderLine::Note(IS_REPO)),
            Some(f) if f.has_files => Some(FolderLine::Note(HAS_FILES)),
            Some(_) => None,
        }
    }

    /// Checks every field. Until the folder text has been checked, Create is blocked silently.
    pub fn problems(&self) -> InitProblems {
        let mut p = InitProblems::default();
        match self.folder_line() {
            Some(FolderLine::Problem(t)) => {
                p.folder = Some(t.into());
                p.blocked = true;
            }
            _ if self.facts().is_none() => p.blocked = true,
            _ => {}
        }
        let branch = self.branch.trim();
        if !branch.is_empty() && !glowfly_core::branch_ops::is_valid_branch_name(branch) {
            p.branch = Some(BRANCH_INVALID.into());
            p.blocked = true;
        }
        if self.license.is_some_and(License::needs_holder)
            && let Err(e) = check_holder(&self.holder)
        {
            p.holder = Some(e.message().into());
            p.blocked = true;
        }
        p
    }

    /// The note about a license file the folder already has.
    pub fn license_kept(&self) -> Option<String> {
        self.facts().and_then(|f| f.license.as_ref()).map(|name| format!("This folder already has {name}; it is kept."))
    }

    /// The files to write, `year` going in a license's copyright line. A `.gitignore` or license
    /// the folder already has is left out.
    pub fn files(&self, year: i32) -> Vec<NewFile> {
        let facts = self.facts();
        let mut out = Vec::new();
        if !facts.is_some_and(|f| f.gitignore)
            && let Some(contents) = gitignore_text(self.gitignore, self.os_editor)
        {
            out.push(NewFile { name: ".gitignore".into(), contents });
        }
        if let Some(l) = self.license
            && facts.is_none_or(|f| f.license.is_none())
        {
            let holder = check_holder(&self.holder).unwrap_or_default();
            out.push(NewFile { name: LICENSE_FILE.into(), contents: license_text(l, year, &holder) });
        }
        out
    }

    /// Whether the commit option applies: something is written into a folder that isn't a
    /// repository yet.
    pub fn can_commit(&self, year: i32) -> bool {
        !self.files(year).is_empty() && !self.facts().is_some_and(|f| f.repo)
    }

    /// The request the form describes, or None while it is blocked.
    pub fn request(&self, year: i32) -> Option<InitRequest> {
        if self.problems().blocked {
            return None;
        }
        let folder = self.facts()?.path.clone();
        let branch = Some(self.branch.trim()).filter(|b| !b.is_empty()).map(str::to_owned);
        let commit = (self.commit && self.can_commit(year)).then(|| INITIAL_COMMIT.to_owned());
        Some(InitRequest { folder, plan: InitPlan { branch, files: self.files(year), commit } })
    }
}
