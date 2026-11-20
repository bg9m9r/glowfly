//! The issue panel's and the Create issue dialog's calls to the tracker (`Effect::Issue`): an
//! issue's details, the dialog's choices, creating an issue and posting a comment. They sign in
//! the way the ISSUES list does; only creating and commenting, which the user asked for, may
//! prompt for credentials.

use super::execute::{Cx, Execute};
use super::sign_ins::{Ask, sign_in};
use glowfly_core::forge::{self, Auth};
use glowfly_model::effect::{ForgeError, IssueCall, IssueEffect, IssueJob, IssueReply};
use glowfly_model::msg::{IssueMsg, Msg};

pub(super) fn run(e: IssueEffect, cx: &Cx<'_>) -> Msg {
    match e {
        IssueEffect::Call(job) => call(*job, cx),
    }
}

impl Execute for IssueEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self, cx))
    }
}

fn call(job: IssueJob, cx: &Cx<'_>) -> Msg {
    let IssueJob { req, generation, call } = job;
    let interactive = matches!(call, IssueCall::Create(_) | IssueCall::Comment { .. });
    let auth: Result<Auth, ForgeError> = match req.auth {
        Some(a) => Ok(a),
        None => {
            let ask = Ask {
                account: req.target.account(),
                cwd: &req.cwd,
                interactive,
                connected: req.connected,
                env_only: false,
                asked: interactive,
            };
            sign_in(cx, ask).map_err(ForgeError::from)
        }
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
        IssueCall::Comment { key, body } => {
            let result = auth.and_then(|a| forge::post_issue_comment(t, &a, &key, &body).map_err(ForgeError::from));
            IssueReply::Commented { key, result }
        }
    };
    Msg::Issue(IssueMsg::Replied { workdir: req.workdir, target: req.target, generation, reply: Box::new(reply) })
}

#[cfg(test)]
mod tests {
    use glowfly_core::forge::{Provider, Target};
    use glowfly_model::effect::{Effect, IssueCall, IssueEffect, IssueJob, IssueRequest};
    use std::path::PathBuf;

    #[test]
    fn issue_calls_wait_in_no_repository_queue() {
        let target =
            Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
        let req = IssueRequest { workdir: PathBuf::from("repo"), cwd: PathBuf::from("repo"), target, auth: None, connected: false };
        let e = Effect::Issue(IssueEffect::Call(Box::new(IssueJob { req, generation: 1, call: IssueCall::Load("1".into()) })));
        assert_eq!(e.queue(), None);
    }
}
