//! The pull request panel's work (`Effect::PullRequest`): the files and commits it brings
//! (`repo::pr_compare`, from the target's remote-tracking tip to the listed head when it is
//! here, else to the remote-tracking source branch), the fetch of the service's head ref into
//! `refs/glowfly/pr/<id>` and keeping or deleting those refs (`pr_head_refs`, best effort: a
//! failure only reaches the debug log), the threads' lines mapped from the service's head onto the
//! local source tip (`repo::pr_line_maps`), and its calls to the hosting service (the page, a comment, a
//! review, its checks, a merge, a thread reply or resolve, a new code comment on diff lines,
//! alone or in the pending review). The calls sign in the way the pull
//! request list does, in the request's `cwd` (the panel's repository while it is trusted).

use super::execute::{Cx, Execute};
use super::sign_ins::{Ask, sign_in};
use glowfly_core::forge::{self, Auth, CheckQuery, ReviewRequest};
use glowfly_core::repo;
use glowfly_model::effect::ForgeError;
use glowfly_model::msg::Msg;
use glowfly_model::pull_request::{CallJob, HeadRefJob, PrCall, PrEffect, PrReply, PullRequestMsg};

pub(super) fn run(e: PrEffect, cx: &Cx<'_>) -> Option<Msg> {
    Some(match e {
        PrEffect::Files(job) => {
            let result = repo::pr_compare(&job.workdir, job.target, job.branch, job.head.as_deref()).map_err(|e| e.to_string());
            Msg::PullRequest(PullRequestMsg::FilesLoaded { id: job.id, generation: job.generation, result })
        }
        PrEffect::FetchHead(job) => {
            let (ok, output) = super::git_ops::run_git(&job.workdir, std::slice::from_ref(&job.cmd), None, job.prompts);
            let result = if ok { Ok(()) } else { Err(glowfly_core::logs::last_message(&output)) };
            Msg::PullRequest(PullRequestMsg::HeadFetched { id: job.id, head: job.head, result })
        }
        PrEffect::LineMaps(job) => {
            let result =
                repo::pr_line_maps(&job.workdir, &job.head, job.local, job.target, job.base, &job.paths).map_err(|e| e.to_string());
            Msg::PullRequest(PullRequestMsg::LinesMapped { id: job.id, generation: job.generation, result })
        }
        PrEffect::HeadRef(job) => {
            head_ref(job);
            return None;
        }
        PrEffect::Call(job) => call(*job, cx),
    })
}

/// Keeps, deletes or sweeps the refs under `refs/glowfly/pr/`. A failure is only logged: the ref
/// a failed delete leaves is swept when the repository opens.
fn head_ref(job: HeadRefJob) {
    use glowfly_core::pr_head_refs::{keep_head, remove_heads, sweep_heads};
    match job {
        HeadRefJob::Keep { workdir, id, oid } => {
            if let Err(e) = keep_head(&workdir, id, oid) {
                tracing::warn!("couldn't keep pull request {id}'s head in {}: {e}", workdir.display());
            }
        }
        HeadRefJob::Drop { workdir, ids } => {
            if let Err(e) = remove_heads(&workdir, &ids) {
                tracing::debug!("couldn't delete the refs of pull requests {ids:?} in {}: {e}", workdir.display());
            }
        }
        HeadRefJob::Sweep { workdir, keep } => match sweep_heads(&workdir, &keep) {
            Ok(n) => tracing::debug!("swept {n} pull request head refs in {}", workdir.display()),
            Err(e) => tracing::debug!("couldn't sweep the pull request head refs in {}: {e}", workdir.display()),
        },
    }
}

impl Execute for PrEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        run(self, cx)
    }
}

fn call(job: CallJob, cx: &Cx<'_>) -> Msg {
    let CallJob { req, id, generation, call } = job;
    // Loading the page or the checks in the background never asks for credentials; the user's
    // own actions may.
    let interactive = !matches!(call, PrCall::Load | PrCall::Checks { .. });
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
        PrCall::Load => PrReply::Loaded(auth.and_then(|a| forge::pr_page(t, &a, id).map_err(ForgeError::from))),
        PrCall::Checks { head } => {
            PrReply::Checks(auth.and_then(|a| forge::check_list(t, &a, &CheckQuery { id, head }).map_err(ForgeError::from)))
        }
        PrCall::Comment(body) => PrReply::Commented(auth.and_then(|a| forge::post_comment(t, &a, id, &body).map_err(ForgeError::from))),
        PrCall::Review { event, body, pending, head_sha } => {
            let req = ReviewRequest { event, body, pending, head_sha };
            PrReply::Reviewed(event, auth.and_then(|a| forge::submit_review(t, &a, id, &req).map_err(ForgeError::from)))
        }
        PrCall::Merge { req: merge, remote, source, delete, delete_local } => {
            let result = auth.and_then(|a| forge::merge(t, &a, id, &merge).map_err(ForgeError::from));
            PrReply::Merged { method: merge.method, remote, source, delete, delete_local, result }
        }
        PrCall::Reply { thread, comment, body } => PrReply::RepliedToThread {
            thread,
            result: auth.and_then(|a| forge::reply_to_thread(t, &a, id, comment, &body).map_err(ForgeError::from)),
        },
        PrCall::Resolve { thread, resolve } => {
            let result = auth.and_then(|a| forge::resolve_thread(t, &a, &thread, resolve).map_err(ForgeError::from));
            PrReply::Resolved { thread, resolve, result }
        }
        PrCall::LineComment(c) => PrReply::LineCommented {
            review: false,
            result: auth.and_then(|a| forge::post_line_comment(t, &a, id, &c).map_err(ForgeError::from)),
        },
        PrCall::ReviewComment { comment, pending } => PrReply::LineCommented {
            review: true,
            result: auth.and_then(|a| forge::add_review_comment(t, &a, id, pending, &comment).map_err(ForgeError::from)),
        },
    };
    Msg::PullRequest(PullRequestMsg::Replied { workdir: req.workdir, target: req.target, id, generation, reply: Box::new(reply) })
}
