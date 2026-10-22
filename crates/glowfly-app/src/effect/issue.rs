//! The issue panel's and the Create issue dialog's calls to the issue tracker
//! (`Effect::Issue`, run by runner/issues.rs): one issue's details, the dialog's choices, and
//! creating an issue and posting a comment. Unscoped: the reply names its repository and tracker.

use super::ForgeError;
use glowfly_core::forge::{Auth, CreateOptions, Entry, Issue, IssueDetails, NewIssue, Target};
use std::path::PathBuf;

/// Where a call goes, with this session's credential for it, if any; `Debug` hides the
/// credential (Auth's own `Debug`).
#[derive(Debug)]
pub struct IssueRequest {
    pub workdir: PathBuf,
    pub target: Target,
    /// None signs in (environment, saved token, Git Credential Manager).
    pub auth: Option<Auth>,
    /// The user connected this account in Preferences.
    pub connected: bool,
}

/// What a call does.
#[derive(Debug)]
pub enum IssueCall {
    /// The details of the issue with this key.
    Load(String),
    /// The Create issue dialog's labels, people, types and epics for this Jira project or Trello
    /// board (Trello, empty: the boards to choose from).
    Options(String),
    Create(Box<NewIssue>),
    /// Post body as a comment on the issue with this key.
    Comment {
        key: String,
        body: String,
    },
}

#[derive(Debug)]
pub struct IssueJob {
    pub req: IssueRequest,
    pub generation: u64,
    pub call: IssueCall,
}

#[derive(Debug)]
pub enum IssueEffect {
    Call(Box<IssueJob>),
}

impl IssueEffect {
    /// Unscoped: the reply names its repository, and its handler matches it.
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            IssueEffect::Call(_) => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            IssueEffect::Call(j) => match j.call {
                IssueCall::Load(_) => "load-issue",
                IssueCall::Options(_) => "load-issue-options",
                IssueCall::Create(_) => "create-issue",
                IssueCall::Comment { .. } => "post-issue-comment",
            },
        }
    }
}

/// What a call answered.
#[derive(Clone, Debug, PartialEq)]
pub enum IssueReply {
    Loaded {
        key: String,
        result: Result<IssueDetails, ForgeError>,
    },
    Options {
        project: String,
        result: Result<CreateOptions, ForgeError>,
    },
    Created(Result<Issue, ForgeError>),
    /// The comment posted on the issue with this key, as the service has it.
    Commented {
        key: String,
        result: Result<Entry, ForgeError>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::forge::Provider;

    #[test]
    fn names_differ_and_nothing_is_scoped() {
        let target =
            Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
        let new = NewIssue {
            project: String::new(),
            title: "t".into(),
            description: String::new(),
            labels: Vec::new(),
            assignees: Vec::new(),
            kind: None,
            priority: None,
            epic: None,
        };
        let calls = [
            IssueCall::Load("1".into()),
            IssueCall::Options(String::new()),
            IssueCall::Create(Box::new(new)),
            IssueCall::Comment { key: "1".into(), body: "Thanks".into() },
        ];
        let effects: Vec<IssueEffect> = calls
            .into_iter()
            .map(|call| {
                let req = IssueRequest { workdir: PathBuf::from("repo"), target: target.clone(), auth: None, connected: false };
                IssueEffect::Call(Box::new(IssueJob { req, generation: 1, call }))
            })
            .collect();
        let names: Vec<&str> = effects.iter().map(IssueEffect::name).collect();
        assert_eq!(names, ["load-issue", "load-issue-options", "create-issue", "post-issue-comment"]);
        assert!(effects.iter().all(|e| e.scope().is_none()));
        let e = crate::effect::Effect::Issue(effects.into_iter().next().unwrap());
        assert_eq!((e.name(), e.scope(), e.timed()), ("load-issue", None, true));
    }
}
