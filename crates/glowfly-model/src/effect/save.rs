//! Saves of Glowfly's own files (`Effect::Save`, runner/saves.rs): the settings, the recent
//! repositories and the open tabs. Unscoped; each file's saves are ordered by runner/saves.rs,
//! and the whole-app tests' runner writes them into the test's folder (`SandboxRule::Save`).

use super::EffectMeta;
use super::SandboxRule;
use glowfly_core::settings::Settings;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum SaveEffect {
    Settings(Box<Settings>),
    Recent(Vec<PathBuf>),
    /// The open tabs (repositories), their labels (parallel to `paths`) and which one is
    /// active, for the next start.
    Tabs {
        paths: Vec<PathBuf>,
        labels: Vec<Option<String>>,
        active: usize,
    },
}

impl EffectMeta for SaveEffect {
    fn name(&self) -> &'static str {
        match self {
            SaveEffect::Settings(_) => "save-settings",
            SaveEffect::Recent(_) => "save-recent",
            SaveEffect::Tabs { .. } => "save-tabs",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    /// Glowfly's own files, never a repository's.
    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Save
    }
}
