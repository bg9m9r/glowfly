//! Keeps a panicking effect from leaving the UI waiting forever: the panic is caught and turned
//! into the failure result the effect's sender is waiting for (a finished job clears `st.job`).

use crate::effect::{Effect, LoadKind, UndoResult};
use crate::msg::JobMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

/// What to report if an effect panics, worked out before it runs.
pub(super) struct Recovery {
    name: &'static str,
    scope: Option<PathBuf>,
    kind: Kind,
}

enum Kind {
    /// A job that holds `st.job`: it must finish, failed, with this undo outcome.
    Job(Option<UndoResult>),
    Rebase,
    LocalOp,
    /// A repository load or status read: the UI waits for it before starting another.
    Load(u64, LoadKind),
    Status(u64),
    Log,
}

impl Recovery {
    pub(super) fn of(effect: &Effect, name: &'static str) -> Self {
        let kind = match effect {
            Effect::RunGit { record, .. } => Kind::Job(record.as_ref().map(|_| UndoResult::Recorded(None))),
            Effect::UndoRedo { redo, .. } => Kind::Job(Some(UndoResult::Failed { redo: *redo })),
            Effect::RemoveWorktree(_) => Kind::Job(None),
            Effect::InteractiveRebase { .. } => Kind::Rebase,
            Effect::LocalOp { .. } => Kind::LocalOp,
            Effect::LoadRepo { generation, kind, .. } => Kind::Load(*generation, *kind),
            Effect::LoadStatus { generation, .. } => Kind::Status(*generation),
            _ => Kind::Log,
        };
        Self { name, scope: effect.scope().cloned(), kind }
    }

    fn message(self, panic: String) -> Msg {
        let text = format!("Trellis hit an internal error ({}): {panic}", self.name);
        let msg = match self.kind {
            Kind::Job(undo) => Msg::Job(JobMsg::JobDone { ok: false, output: text, undo }),
            Kind::Rebase => Msg::Rebase(RebaseMsg::RebaseFinished { result: Err(text) }),
            Kind::LocalOp => Msg::Stage(StageMsg::LocalOpDone(Err(text))),
            Kind::Load(generation, kind) => Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result: Err(text) }),
            Kind::Status(generation) => {
                Msg::Load(LoadMsg::StatusLoaded { generation, result: Err(text), submodules: None, open_stamp: None })
            }
            Kind::Log => Msg::Logged { ok: false, text },
        };
        match self.scope {
            Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
            None => msg,
        }
    }
}

/// Runs `f`, turning a panic into `recovery`'s failure message (and logging it).
pub(super) fn guarded(recovery: Recovery, f: impl FnOnce() -> Option<Msg>) -> Option<Msg> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(msg) => msg,
        Err(payload) => {
            let panic = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            tracing::error!(effect = recovery.name, "effect panicked: {panic}");
            Some(recovery.message(panic))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, RunningJob};
    use crate::update::update;
    use std::time::Instant;
    use trellis_core::settings::Settings;
    use trellis_core::undo::UndoKind;

    #[test]
    fn a_panicking_job_finishes_failed_and_frees_the_job_slot() {
        let effect = Effect::RunGit {
            cwd: "C:/r".into(),
            cmds: vec![],
            write_file: None,
            record: Some(UndoKind::Checkout),
            prompts: false,
            expect: None,
        };
        let msg = guarded(Recovery::of(&effect, "git"), || panic!("boom {}", 42)).expect("a failure result");
        let Msg::Job(JobMsg::JobDone { ok: false, output, undo: Some(UndoResult::Recorded(None)) }) = &msg else {
            panic!("not a failed job")
        };
        assert!(output.contains("boom 42") && output.contains("git"), "{output}");

        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        st.job = Some(RunningJob {
            label: "Pull".into(),
            workdir: None,
            quiet: false,
            clear_commit: false,
            then_open: None,
            then_push: false,
            keeps_undo: false,
        });
        update(&mut st, msg, Instant::now());
        assert!(st.job.is_none(), "the UI can start another operation");
    }

    #[test]
    fn a_panicking_local_op_reports_to_its_repository() {
        let effect = Effect::LocalOp { workdir: "C:/r".into(), op: crate::effect::LocalOp::StageAll };
        let msg = guarded(Recovery::of(&effect, "local-op"), || panic!("static")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Stage(StageMsg::LocalOpDone(Err(ref e))) if e.contains("static")));
        assert!(guarded(Recovery::of(&effect, "local-op"), || None).is_none(), "no panic, no message");
    }

    #[test]
    fn a_panicking_load_ends_the_load() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        st.load_gen = 3;
        st.loading = true;
        let effect =
            Effect::LoadRepo { path: "C:/r".into(), opts: Default::default(), generation: 3, kind: LoadKind::Refresh, known: None };
        let msg = guarded(Recovery::of(&effect, "load-repo"), || panic!("boom")).unwrap();
        assert!(matches!(&msg, Msg::Repo(RepoMsg::RepoLoaded { generation: 3, result: Err(e), .. }) if e.contains("boom")));
        update(&mut st, msg, Instant::now());
        assert!(!st.loading, "the next refresh isn't left waiting for it");

        let effect = Effect::LoadStatus { workdir: "C:/r".into(), generation: 3, submodules: false, open: None };
        let msg = guarded(Recovery::of(&effect, "load-status"), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Load(LoadMsg::StatusLoaded { generation: 3, result: Err(_), .. })));
    }
}
