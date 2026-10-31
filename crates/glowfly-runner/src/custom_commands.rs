//! Custom commands on a worker thread: reading both commands files, and running a shell command
//! as the job (its end is an ordinary `JobDone`, finished by `ops::finish_job`).

use super::execute::{Cx, Execute};
use crate::msg::{JobMsg, Msg};
use glowfly_core::custom_commands;
use glowfly_model::custom_commands::{CustomCommandEffect, CustomCommandMsg};

pub(super) fn run(e: CustomCommandEffect) -> Msg {
    match e {
        CustomCommandEffect::Load { generation, workdir } => {
            let user = custom_commands::read_user();
            let repo = custom_commands::read_repo(&workdir);
            tracing::debug!(
                "custom commands: {} of yours ({} problems), {} of the repository's ({} problems)",
                user.commands.len(),
                user.problems.len(),
                repo.commands.len(),
                repo.problems.len()
            );
            Msg::CustomCommand(CustomCommandMsg::Loaded { generation, workdir, user, repo })
        }
        CustomCommandEffect::RunShell(r) => {
            let (ok, output) = custom_commands::run_shell(&r.cwd, &r.script);
            Msg::Job(JobMsg::JobDone { ok, output, undo: None })
        }
    }
}

impl Execute for CustomCommandEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
