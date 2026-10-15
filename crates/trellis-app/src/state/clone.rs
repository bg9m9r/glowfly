//! The Clone dialog's form: the URL, the parent folder and the shallow clone options, with the
//! pure rules that check them as the user types and the request they turn into.

use chrono::NaiveDate;
use std::path::PathBuf;
use trellis_core::clone_ops::{
    BRANCH_INVALID, CloneOptions, HistoryLimit, Shallow, check_clone_branch, parse_depth, parse_since, repo_name,
};
use trellis_core::remote_ops::{UrlProblem, check_url};

/// How a shallow clone limits the history it downloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind {
    /// The newest commits, by count.
    Depth,
    /// Commits since a date.
    Since,
}

/// The Clone dialog's fields. The depth and since texts are kept separately so switching modes
/// never loses what was typed.
#[derive(Clone, Debug, PartialEq)]
pub struct CloneForm {
    pub url: String,
    pub parent: String,
    pub shallow: bool,
    pub limit: LimitKind,
    pub depth: String,
    pub since: String,
    pub branch: String,
    /// The parent folder text last checked on disk (trimmed) and whether it is a folder. Set
    /// by `update` from `CloneMsg::ParentChecked`; views never look at the disk.
    pub parent_checked: Option<(String, bool)>,
}

/// What is wrong with the form: each field's red line (None when it is fine or empty in a way
/// that needs no words), and whether Clone is blocked (silent problems included).
#[derive(Debug, Default, PartialEq)]
pub struct CloneProblems {
    pub url: Option<String>,
    pub folder: Option<String>,
    pub depth: Option<String>,
    pub since: Option<String>,
    pub branch: Option<String>,
    pub blocked: bool,
}

/// A clone to run: `url` cloned into `parent`/`name`.
#[derive(Clone, Debug, PartialEq)]
pub struct CloneRequest {
    pub url: String,
    pub parent: PathBuf,
    pub name: String,
    pub options: CloneOptions,
}

impl CloneForm {
    /// An empty form cloning into `parent`; shallow is off, with a depth of 1 ready.
    pub fn new(parent: String) -> Self {
        Self {
            url: String::new(),
            parent,
            shallow: false,
            limit: LimitKind::Depth,
            depth: "1".into(),
            since: String::new(),
            branch: String::new(),
            parent_checked: None,
        }
    }

    /// Whether the parent folder exists, by the last check of the text now in the field; None
    /// until that text has been checked.
    pub fn parent_is_dir(&self) -> Option<bool> {
        self.parent_checked.as_ref().filter(|(text, _)| text == self.parent.trim()).map(|&(_, is_dir)| is_dir)
    }

    /// `problems` with the last check of the parent folder. Until the text in the field has
    /// been checked, Clone is blocked without a message.
    pub fn problems_now(&self, today: NaiveDate) -> CloneProblems {
        match self.parent_is_dir() {
            Some(is_dir) => self.problems(today, is_dir),
            None => CloneProblems { blocked: true, ..self.problems(today, true) },
        }
    }

    /// The folder name the clone creates.
    pub fn name(&self) -> String {
        repo_name(&self.url)
    }

    /// Checks every field. `today` is the local date (the latest start date allowed);
    /// `parent_is_dir` says whether the parent folder exists.
    pub fn problems(&self, today: NaiveDate, parent_is_dir: bool) -> CloneProblems {
        let mut p = CloneProblems::default();
        let url = self.url.trim();
        match check_url(url) {
            Err(UrlProblem::Empty) => p.blocked = true,
            Err(e) => {
                p.url = Some(e.message(false));
                p.blocked = true;
            }
            Ok(()) if self.name().is_empty() => {
                p.url = Some("Can't tell the new folder's name from this URL.".into());
                p.blocked = true;
            }
            Ok(()) => {}
        }
        if self.parent.trim().is_empty() {
            p.blocked = true;
        } else if !parent_is_dir {
            p.folder = Some("That folder doesn't exist.".into());
            p.blocked = true;
        }
        if self.shallow {
            match self.limit {
                LimitKind::Depth => {
                    if let Err(e) = parse_depth(&self.depth) {
                        p.depth = Some(e.message().into());
                        p.blocked = true;
                    }
                }
                LimitKind::Since => {
                    if let Err(e) = parse_since(&self.since, today) {
                        p.since = Some(e.message().into());
                        p.blocked = true;
                    }
                }
            }
            if check_clone_branch(&self.branch).is_err() {
                p.branch = Some(BRANCH_INVALID.into());
                p.blocked = true;
            }
        }
        p
    }

    /// The request the form describes, or None when a field doesn't parse.
    pub fn request(&self, today: NaiveDate) -> Option<CloneRequest> {
        let shallow = if self.shallow {
            let limit = match self.limit {
                LimitKind::Depth => HistoryLimit::Depth(parse_depth(&self.depth).ok()?),
                LimitKind::Since => HistoryLimit::Since(parse_since(&self.since, today).ok()?),
            };
            Some(Shallow { limit, branch: check_clone_branch(&self.branch).ok()? })
        } else {
            None
        };
        let name = self.name();
        if name.is_empty() {
            return None;
        }
        Some(CloneRequest {
            url: self.url.trim().to_owned(),
            parent: PathBuf::from(self.parent.trim()),
            name,
            options: CloneOptions { shallow },
        })
    }
}
