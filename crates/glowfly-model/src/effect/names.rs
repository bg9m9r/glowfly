//! Short names of effects, for the worker threads and the debug log.

use super::areas::{effect_areas, effect_match};
use super::{Effect, EffectMeta};

impl Effect {
    /// A short name for the worker thread and the debug log, from the area's `EffectMeta`.
    pub fn name(&self) -> &'static str {
        effect_areas! { effect_match! { self, |e| e.name() } }
    }

    /// Whether the runner times this effect for the performance log (runner/mod.rs), from the
    /// area's `EffectMeta`.
    pub fn timed(&self) -> bool {
        effect_areas! { effect_match! { self, |e| e.timed() } }
    }
}

#[cfg(test)]
mod tests {
    use super::Effect;
    use crate::effect::EffectMeta;
    use crate::patch::PatchEffect;
    use crate::ssh::SshEffect;
    use std::path::PathBuf;

    #[test]
    fn effects_that_wait_on_the_user_are_not_timed() {
        let dir = PathBuf::from("repo");
        let oids = vec![git2::Oid::zero()];
        let what = glowfly_core::patch_ops::PatchSource::Commits { oids: oids.clone(), paths: Vec::new() };
        let pick_save = PatchEffect::PickSave { workdir: dir.clone(), what: what.clone(), name: "a.patch".into() };
        assert!(!Effect::Patch(pick_save).timed());
        assert!(!Effect::Patch(PatchEffect::PickOpen { workdir: dir.clone() }).timed());
        assert!(Effect::Patch(PatchEffect::Export { workdir: dir.clone(), what, dest: dir.join("a.patch") }).timed());
        let inspect = PatchEffect::Inspect { workdir: dir.clone(), path: dir.join("a.patch") };
        assert_eq!(inspect.name(), "inspect-patch");
        assert!(Effect::Patch(inspect).timed());
        let copy = PatchEffect::Copy { workdir: dir.clone(), oids };
        assert_eq!(copy.name(), "copy-patch");
        assert!(Effect::Patch(copy).timed());
        assert!(!Effect::Ssh(SshEffect::AddToAgent { ssh_add: "ssh-add".into(), key: dir.join("id") }).timed());
        assert!(Effect::Ssh(SshEffect::Agent { generation: 1 }).timed());
        let windows = SshEffect::SetWindowsSsh { program: None };
        assert_eq!(windows.name(), "ssh-windows");
        assert!(Effect::Ssh(windows).timed());
        assert!(Effect::Ssh(SshEffect::List { generation: 1, extra: Vec::new() }).timed());
        let pick = SshEffect::PickKey { title: "Choose a private key".into(), start: dir.clone(), keygen: None };
        assert_eq!(pick.name(), "ssh-pick-key");
        assert!(!Effect::Ssh(pick).timed(), "waits on the user");
        let export = Effect::Logs(crate::logs::LogsEffect::PickExport { name: "x".into() });
        assert_eq!(export.name(), "pick-logs-export");
        assert!(!export.timed(), "waits on the user");
    }

    #[test]
    fn custom_command_effects_are_named_and_timed() {
        use crate::custom_commands::{CustomCommandEffect, ShellRun};
        let load = Effect::CustomCommand(CustomCommandEffect::Load { generation: 1, workdir: PathBuf::from("repo") });
        assert_eq!((load.name(), load.timed()), ("load-custom-commands", true));
        let shell = Effect::CustomCommand(CustomCommandEffect::RunShell(ShellRun { cwd: PathBuf::from("repo"), script: "make".into() }));
        assert_eq!((shell.name(), shell.timed()), ("run-custom-command", true));
    }
}
