//! The issue panel's and the Create issue dialog's calls to the tracker (`Effect::Issue`): an
//! issue's details, the dialog's choices and creating an issue. They sign in the way the
//! ISSUES list does; only creating, which the user asked for, may prompt for credentials.

use crate::effect::{ForgeError, IssueCall, IssueEffect, IssueJob, IssueReply};
use crate::msg::{IssueMsg, Msg};
use trellis_core::forge::{self, Auth};

pub(super) fn run(e: IssueEffect) -> Msg {
    match e {
        IssueEffect::Call(job) => call(*job),
    }
}

fn call(job: IssueJob) -> Msg {
    let IssueJob { req, generation, call } = job;
    let interactive = matches!(call, IssueCall::Create(_));
    let auth: Result<Auth, ForgeError> = match req.auth {
        Some(a) => Ok(a),
        None => forge::resolve_auth(&req.target.account(), &req.workdir, interactive, req.connected).map_err(ForgeError::from),
    };
    let t = &req.target;
    let reply = match call {
        IssueCall::Load(key) => {
            let result = auth.and_then(|a| forge::issue_details(t, &a, &key).map_err(ForgeError::from));
            IssueReply::Loaded { key, result }
        }
        IssueCall::Options(project) => {
            let result = auth.and_then(|a| forge::issue_create_options(t, &a, &project).map_err(ForgeError::from));
            IssueReply::Options { project, result }
        }
        IssueCall::Create(new) => IssueReply::Created(auth.and_then(|a| forge::create_issue(t, &a, &new).map_err(ForgeError::from))),
    };
    Msg::Issue(IssueMsg::Replied { workdir: req.workdir, target: req.target, generation, reply: Box::new(reply) })
}

#[cfg(test)]
mod tests {
    use crate::effect::{Effect, IssueCall, IssueEffect, IssueJob, IssueRequest};
    use std::path::PathBuf;
    use trellis_core::forge::{Provider, Target};

    #[test]
    fn issue_calls_wait_in_no_repository_queue() {
        let target =
            Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
        let req = IssueRequest { workdir: PathBuf::from("repo"), target, auth: None, connected: false };
        let e = Effect::Issue(IssueEffect::Call(Box::new(IssueJob { req, generation: 1, call: IssueCall::Load("1".into()) })));
        assert_eq!(super::super::queue::repo_of(&e), None);
    }
}
